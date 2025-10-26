pub mod batch;
pub mod stage_dispatch;
pub mod status_handlers;

use std::{collections::HashMap, path::PathBuf};

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    app::config::Config,
    processing::{
        coordinator::{
            batch::{BatchId, ProcessingBatch, handle_new_batch},
            stage_dispatch::handle_stage_dispatch,
            status_handlers::handle_track_status,
        },
        stages::{ai, beatport, keyfinder, prepare_media},
    },
};

/// Coordinates processing of multiple batches through all stages
pub struct ProcessingCoordinator {
    batch_sender: mpsc::UnboundedSender<ProcessingBatch>,
    main_loop_handle: JoinHandle<()>,
}

impl ProcessingCoordinator {
    pub fn start(config: &Config) -> Self {
        let mut batches: HashMap<BatchId, ProcessingBatch> = HashMap::new();

        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();
        let (status_update_tx, mut status_update_rx) = mpsc::unbounded_channel();
        let (batch_sender, mut batch_rx) = mpsc::unbounded_channel();

        // Create and start all processors
        let prepare_media_processor = prepare_media::new_prepare_media_processor();
        let prepare_media_sender = prepare_media_processor.get_sender();
        tokio::spawn(prepare_media_processor.start());

        let keyfinder_processor = keyfinder::new_keyfinder_processor();
        let keyfinder_sender = keyfinder_processor.get_sender();
        tokio::spawn(keyfinder_processor.start());

        let beatport_processor = beatport::new_beatport_processor(config.beatport.as_ref());
        let beatport_sender = beatport_processor.get_sender();
        tokio::spawn(beatport_processor.start());

        let ai_processor = ai::new_ai_processor(config.ai.as_ref());
        let ai_sender = ai_processor.get_sender();
        tokio::spawn(ai_processor.start());

        // Start the status handler loop and wait for completion
        let main_loop_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(batch) = batch_rx.recv() =>
                        handle_new_batch(&mut batches, &stage_dispatch_tx, batch),
                    Some(track_status) = status_update_rx.recv() =>
                        handle_track_status(&mut batches, &stage_dispatch_tx, track_status),
                    Some(input) = stage_dispatch_rx.recv() =>
                        handle_stage_dispatch(
                            input,
                            &prepare_media_sender,
                            &keyfinder_sender,
                            &beatport_sender,
                            &ai_sender,
                            &status_update_tx,
                        ),
                    else => break,
                }
            }
        });

        Self {
            batch_sender,
            main_loop_handle,
        }
    }

    /// Create and process a batch of files
    pub fn process_batch(&self, files: Vec<PathBuf>) {
        let _ = self.batch_sender.send(ProcessingBatch::new(files));
    }

    pub async fn await_completion(self) {
        let _ = self.main_loop_handle.await;
    }
}
