pub mod batch;
pub mod callbacks;
pub mod stage_dispatcher;
pub mod stage_runner;
pub mod stage_status;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::app::config::Config;

use super::stages::{ai, beatport, keyfinder, prepare_media};

use self::{
    batch::{BatchHandle, BatchId, BatchState, ProcessingBatch, handle_new_batch},
    callbacks::{CallbackHandle, CallbackRegistry, StatusCallback},
    stage_runner::handle_stage_dispatch,
    stage_status::handle_track_status,
};

/// Holds all stage processor senders
pub struct StageProcessors {
    pub prepare_media_sender: prepare_media::PrepareMediaSender,
    pub keyfinder_sender: keyfinder::KeyfinderSender,
    pub beatport_sender: beatport::BeatportSender,
    pub ai_sender: ai::AiSender,
}

impl StageProcessors {
    /// Boot all stage processors and return their senders
    pub fn boot(config: &Config) -> Self {
        // Create and start prepare_media processor
        let prepare_media_processor = prepare_media::new_prepare_media_processor();
        let prepare_media_sender = prepare_media_processor.get_sender();
        tokio::spawn(prepare_media_processor.start());

        // Create and start keyfinder processor
        let keyfinder_processor = keyfinder::new_keyfinder_processor();
        let keyfinder_sender = keyfinder_processor.get_sender();
        tokio::spawn(keyfinder_processor.start());

        // Create and start beatport processor
        let beatport_processor = beatport::new_beatport_processor(config.beatport.as_ref());
        let beatport_sender = beatport_processor.get_sender();
        tokio::spawn(beatport_processor.start());

        // Create and start AI processor
        let ai_processor = ai::new_ai_processor(config.ai.as_ref());
        let ai_sender = ai_processor.get_sender();
        tokio::spawn(ai_processor.start());

        Self {
            prepare_media_sender,
            keyfinder_sender,
            beatport_sender,
            ai_sender,
        }
    }
}

/// Coordinates processing of multiple batches through all stages
pub struct ProcessingCoordinator {
    batch_sender: mpsc::UnboundedSender<ProcessingBatch>,
    callback_registry: Arc<CallbackRegistry>,
    cancellation_token: CancellationToken,
    main_loop_handle: JoinHandle<()>,
}

impl ProcessingCoordinator {
    pub fn start(config: &Config) -> Self {
        let mut batches: HashMap<BatchId, ProcessingBatch> = HashMap::new();

        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();
        let (status_update_tx, mut status_update_rx) = mpsc::unbounded_channel();
        let (batch_sender, mut batch_rx) = mpsc::unbounded_channel();
        let callback_registry = Arc::new(CallbackRegistry::new());
        let cancellation_token = CancellationToken::new();

        // Boot all stage processors
        let stage_processors = StageProcessors::boot(config);

        let token_clone = cancellation_token.clone();
        let cb_registry = callback_registry.clone();

        let main_loop_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token_clone.cancelled() => {
                        // Immediate shutdown requested
                        break;
                    }
                    Some(batch) = batch_rx.recv() =>
                        handle_new_batch(&mut batches, &stage_dispatch_tx, batch),
                    Some(track_status) = status_update_rx.recv() =>
                        handle_track_status(&mut batches, &stage_dispatch_tx, &cb_registry, track_status),
                    Some(input) = stage_dispatch_rx.recv() =>
                        handle_stage_dispatch(input, &stage_processors, &status_update_tx),
                    else => break,
                }

                if batch_rx.is_closed()
                    && batches
                        .values()
                        .all(|b| matches!(b.state, BatchState::Complete))
                {
                    break;
                }
            }
        });

        Self {
            batch_sender,
            callback_registry,
            cancellation_token,
            main_loop_handle,
        }
    }

    /// Create and process a batch of files
    pub fn process_batch(&self, files: Vec<PathBuf>) -> BatchHandle {
        let (completion_tx, completion_rx) = tokio::sync::oneshot::channel();
        let batch = ProcessingBatch::new(files, completion_tx);
        let batch_id = batch.id.clone();

        let _ = self.batch_sender.send(batch);

        BatchHandle::new(batch_id, completion_rx, self.callback_registry.clone())
    }

    /// Stop accepting new batches and wait for all existing batches to complete
    pub async fn shutdown(self) {
        // Drop batch_sender to signal no more batches will be added
        drop(self.batch_sender);

        // Wait for main loop to finish processing all existing batches
        let _ = self.main_loop_handle.await;
    }

    /// Immediately stop all processing and exit
    pub fn force_shutdown(&self) {
        self.cancellation_token.cancel();
    }

    /// Register a callback to receive status events
    pub fn on_status<C: StatusCallback + 'static>(&self, callback: C) -> CallbackHandle {
        self.callback_registry.register(callback)
    }
}
