use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use id3::Tag;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::{
    processing::{
        coordinator::stage_dispatcher::dispatch_next_stages,
        stages::{ProcessingStage, StageInput, StageStatus},
    },
    track::TrackRevision,
};

use super::callbacks::{BatchFilteredCallback, CallbackHandle, CallbackRegistry, StatusCallback};

/// Unique identifier for a processing batch
#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
pub struct BatchId(Uuid);

impl BatchId {
    fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct BatchStageInput {
    pub batch_id: BatchId,
    pub stage_input: StageInput,
}

/// Events emitted during batch processing
#[derive(Debug)]
pub enum StatusEvent<'a> {
    /// A track's stage status was updated
    TrackStageUpdate {
        file_path: PathBuf,
        batch: &'a ProcessingBatch,
        status: Arc<StageStatus>,
        revision: Box<Option<TrackRevision>>,
    },
    /// A batch completed all processing
    BatchCompleted { batch: &'a ProcessingBatch },
}

/// State of a batch's processing lifecycle
#[derive(Debug)]
pub enum BatchState {
    /// Batch is currently processing, holds sender to notify on completion
    Processing(oneshot::Sender<()>),
    /// Batch has completed all processing
    Complete,
}

/// Represents a batch of tracks being processed together
#[derive(Debug)]
pub struct ProcessingBatch {
    pub id: BatchId,

    /// Stages that have been dispatched for this batch (at the [`StageMode::Batch`] level)
    pub stage_dispatched: HashSet<ProcessingStage>,

    /// Map of file paths to their processing state
    pub tracks: HashMap<PathBuf, TrackProcessingState>,

    /// Current state of the batch
    pub state: BatchState,
}

impl ProcessingBatch {
    /// Create a new ProcessingBatch with the given files and completion sender
    pub fn new(files: Vec<PathBuf>, completion_tx: oneshot::Sender<()>) -> Self {
        let mut tracks = HashMap::new();
        for file_path in files {
            let track_state = TrackProcessingState::new(file_path.clone());
            tracks.insert(file_path, track_state);
        }

        Self {
            id: BatchId::new(),
            stage_dispatched: HashSet::new(),
            tracks,
            state: BatchState::Processing(completion_tx),
        }
    }

    /// Check if all tracks in this batch have completed processing
    ///
    /// A track is considered complete if either:
    /// - All stages have completed/been skipped, OR
    /// - Any stage has failed (preventing subsequent stages from running)
    pub fn is_complete(&self) -> bool {
        use strum::IntoEnumIterator;

        self.tracks.values().all(|track| {
            // Track is complete if any stage has failed or if all stages are done
            let has_failure = track.has_failed_stage();
            let all_done = ProcessingStage::iter().all(|stage| track.is_stage_done(&stage));
            has_failure || all_done
        })
    }
}

/// Represents the processing state of a single track
#[derive(Debug)]
pub struct TrackProcessingState {
    // Filepath of the track
    pub file_path: PathBuf,

    /// Stages that have been dispatched for this track
    pub stage_dispatched: HashSet<ProcessingStage>,

    /// Status of each processing stage for this track
    pub stage_status: HashSet<Arc<StageStatus>>,

    /// The tag for the track. Available once [`ProcessingStage::PrepareMedia`] has completed
    pub tag: Option<Tag>,
}

impl TrackProcessingState {
    /// Create a new TrackProcessingState for the given file path
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().into(),
            stage_status: HashSet::new(),
            stage_dispatched: HashSet::new(),
            tag: None,
        }
    }

    pub fn is_stage_done(&self, stage: impl Borrow<ProcessingStage>) -> bool {
        self.get_stage_status(stage)
            .map(|status| status.is_done())
            .unwrap_or(false)
    }

    pub fn get_stage_status(&self, stage: impl Borrow<ProcessingStage>) -> Option<&StageStatus> {
        self.stage_status
            .iter()
            .map(|status| status.as_ref())
            .find(|status| status.stage().eq(stage.borrow()))
    }

    pub fn set_stage_status(&mut self, status: Arc<StageStatus>) {
        self.stage_status.replace(status);
    }

    pub fn has_failed_stage(&self) -> bool {
        self.stage_status.iter().any(|status| status.has_failed())
    }

    /// Check if all prerequisite stages for a given stage are complete and the stage has not
    /// already been dispatched
    pub fn can_run_stage(&self, stage: &ProcessingStage) -> bool {
        // Can't run any stage if a previous stage has failed
        if self.has_failed_stage() {
            return false;
        }

        let stage_ready = match stage {
            ProcessingStage::PrepareMedia => true,
            ProcessingStage::Keyfinder => self.is_stage_done(ProcessingStage::PrepareMedia),
            ProcessingStage::Beatport => self.is_stage_done(ProcessingStage::PrepareMedia),
            ProcessingStage::Ai => self.is_stage_done(ProcessingStage::Beatport),
        };

        stage_ready && !self.stage_dispatched.contains(stage)
    }
}

/// Handle to a processing batch that allows waiting for completion
pub struct BatchHandle {
    batch_id: BatchId,
    completion_rx: oneshot::Receiver<()>,
    callback_registry: Arc<CallbackRegistry>,
}

impl BatchHandle {
    pub fn new(
        batch_id: BatchId,
        completion_rx: oneshot::Receiver<()>,
        callback_registry: Arc<CallbackRegistry>,
    ) -> Self {
        Self {
            batch_id,
            completion_rx,
            callback_registry,
        }
    }

    /// Register a callback that receives events for this specific batch
    pub fn on_status<C: StatusCallback + 'static>(&self, callback: C) -> CallbackHandle {
        self.callback_registry.register(BatchFilteredCallback::new(
            self.batch_id.clone(),
            Arc::new(callback),
        ))
    }

    /// Wait for this batch to complete processing
    pub async fn await_completion(self) -> Result<(), oneshot::error::RecvError> {
        self.completion_rx.await
    }
}

/// Stores and dispatches a new ProcessingBatch
pub fn handle_new_batch(
    batches: &mut HashMap<BatchId, ProcessingBatch>,
    stage_dispatch_tx: &mpsc::UnboundedSender<BatchStageInput>,
    batch: ProcessingBatch,
) {
    let batch_id = batch.id.clone();

    // Register the batch and dispatch the PrepareMedia stage for all tracks
    batches.insert(batch_id.clone(), batch);
    dispatch_next_stages(batches.get_mut(&batch_id).unwrap(), stage_dispatch_tx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::concurrent::ItemStatus;
    use crate::processing::stages::{prepare_media, test_helpers::*, ProcessingStage, StageStatus};
    use std::path::PathBuf;

    #[test]
    fn test_can_run_stage_prepare_media_always_ready() {
        let track = TrackProcessingState::new("/test/track.mp3");
        assert!(track.can_run_stage(&ProcessingStage::PrepareMedia));
    }

    #[test]
    fn test_can_run_stage_keyfinder_requires_prepare_media() {
        let mut track = TrackProcessingState::new("/test/track.mp3");

        assert!(!track.can_run_stage(&ProcessingStage::Keyfinder));

        track.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        assert!(track.can_run_stage(&ProcessingStage::Keyfinder));
    }

    #[test]
    fn test_can_run_stage_beatport_requires_prepare_media() {
        let mut track = TrackProcessingState::new("/test/track.mp3");

        assert!(!track.can_run_stage(&ProcessingStage::Beatport));

        track.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        assert!(track.can_run_stage(&ProcessingStage::Beatport));
    }

    #[test]
    fn test_can_run_stage_ai_requires_beatport() {
        let mut track = TrackProcessingState::new("/test/track.mp3");

        assert!(!track.can_run_stage(&ProcessingStage::Ai));

        // PrepareMedia alone is not enough for AI
        track.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        assert!(!track.can_run_stage(&ProcessingStage::Ai));

        track.set_stage_status(make_status_completed(ProcessingStage::Beatport));
        assert!(track.can_run_stage(&ProcessingStage::Ai));
    }

    #[test]
    fn test_can_run_stage_not_ready_if_already_dispatched() {
        let mut track = TrackProcessingState::new("/test/track.mp3");

        assert!(track.can_run_stage(&ProcessingStage::PrepareMedia));

        track.stage_dispatched.insert(ProcessingStage::PrepareMedia);
        assert!(!track.can_run_stage(&ProcessingStage::PrepareMedia));
    }

    #[test]
    fn test_is_stage_done_for_completed_stage() {
        let mut track = TrackProcessingState::new("/test/track.mp3");

        assert!(!track.is_stage_done(&ProcessingStage::PrepareMedia));

        track.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        assert!(track.is_stage_done(&ProcessingStage::PrepareMedia));
    }

    #[test]
    fn test_is_stage_done_for_skipped_stage() {
        let mut track = TrackProcessingState::new("/test/track.mp3");

        // Skipped stages are considered "done"
        track.set_stage_status(make_status_completed(ProcessingStage::Beatport));
        assert!(track.is_stage_done(&ProcessingStage::Beatport));
    }

    #[test]
    fn test_is_stage_done_for_running_stage() {
        let mut track = TrackProcessingState::new("/test/track.mp3");

        track.set_stage_status(make_status_running(ProcessingStage::PrepareMedia));
        assert!(!track.is_stage_done(&ProcessingStage::PrepareMedia));
    }

    #[test]
    fn test_batch_is_complete_when_all_stages_done() {
        let (tx, _rx) = oneshot::channel();
        let files = vec![PathBuf::from("/test/track1.mp3")];
        let mut batch = ProcessingBatch::new(files, tx);

        assert!(!batch.is_complete());

        let track = batch
            .tracks
            .get_mut(&PathBuf::from("/test/track1.mp3"))
            .unwrap();

        track.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        track.set_stage_status(make_status_completed(ProcessingStage::Keyfinder));
        track.set_stage_status(make_status_completed(ProcessingStage::Beatport));
        track.set_stage_status(make_status_completed(ProcessingStage::Ai));

        assert!(batch.is_complete());
    }

    #[test]
    fn test_batch_is_not_complete_if_one_stage_missing() {
        let (tx, _rx) = oneshot::channel();
        let files = vec![PathBuf::from("/test/track1.mp3")];
        let mut batch = ProcessingBatch::new(files, tx);

        let track = batch
            .tracks
            .get_mut(&PathBuf::from("/test/track1.mp3"))
            .unwrap();

        // Missing AI stage
        track.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        track.set_stage_status(make_status_completed(ProcessingStage::Keyfinder));
        track.set_stage_status(make_status_completed(ProcessingStage::Beatport));

        assert!(!batch.is_complete());
    }

    #[test]
    fn test_batch_is_complete_requires_all_tracks() {
        let (tx, _rx) = oneshot::channel();
        let files = vec![
            PathBuf::from("/test/track1.mp3"),
            PathBuf::from("/test/track2.mp3"),
        ];
        let mut batch = ProcessingBatch::new(files, tx);

        let track1 = batch
            .tracks
            .get_mut(&PathBuf::from("/test/track1.mp3"))
            .unwrap();
        track1.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        track1.set_stage_status(make_status_completed(ProcessingStage::Keyfinder));
        track1.set_stage_status(make_status_completed(ProcessingStage::Beatport));
        track1.set_stage_status(make_status_completed(ProcessingStage::Ai));

        assert!(!batch.is_complete());

        let track2 = batch
            .tracks
            .get_mut(&PathBuf::from("/test/track2.mp3"))
            .unwrap();
        track2.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        track2.set_stage_status(make_status_completed(ProcessingStage::Keyfinder));
        track2.set_stage_status(make_status_completed(ProcessingStage::Beatport));
        track2.set_stage_status(make_status_completed(ProcessingStage::Ai));

        assert!(batch.is_complete());
    }

    #[test]
    fn test_get_stage_status() {
        let mut track = TrackProcessingState::new("/test/track.mp3");

        assert!(
            track
                .get_stage_status(&ProcessingStage::PrepareMedia)
                .is_none()
        );

        track.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));

        let retrieved = track.get_stage_status(&ProcessingStage::PrepareMedia);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().stage(), ProcessingStage::PrepareMedia);
    }

    #[test]
    fn test_batch_completes_when_track_has_failed_stage() {
        let (tx, _rx) = oneshot::channel();
        let files = vec![PathBuf::from("/test/track1.mp3")];
        let mut batch = ProcessingBatch::new(files, tx);

        assert!(!batch.is_complete());

        let track = batch
            .tracks
            .get_mut(&PathBuf::from("/test/track1.mp3"))
            .unwrap();

        // Create a failed PrepareMedia status
        let error = prepare_media::PrepareMediaError::Tag(id3::Error::new(
            id3::ErrorKind::NoTag,
            "Test error",
        ));
        let failed_status = StageStatus::PrepareMedia(ItemStatus::Complete(Err(error)));
        track.set_stage_status(Arc::new(failed_status));

        // Batch should be complete even though only PrepareMedia ran (and failed)
        // The other stages (Keyfinder, Beatport, AI) never ran
        assert!(batch.is_complete());
    }

    #[test]
    fn test_batch_requires_all_tracks_with_mixed_success_and_failure() {
        let (tx, _rx) = oneshot::channel();
        let files = vec![
            PathBuf::from("/test/track1.mp3"),
            PathBuf::from("/test/track2.mp3"),
        ];
        let mut batch = ProcessingBatch::new(files, tx);

        // Track 1 fails at PrepareMedia
        let track1 = batch
            .tracks
            .get_mut(&PathBuf::from("/test/track1.mp3"))
            .unwrap();
        let error = prepare_media::PrepareMediaError::Tag(id3::Error::new(
            id3::ErrorKind::NoTag,
            "Test error",
        ));
        let failed_status = StageStatus::PrepareMedia(ItemStatus::Complete(Err(error)));
        track1.set_stage_status(Arc::new(failed_status));

        // Batch not complete yet - track2 hasn't finished
        assert!(!batch.is_complete());

        // Track 2 succeeds through all stages
        let track2 = batch
            .tracks
            .get_mut(&PathBuf::from("/test/track2.mp3"))
            .unwrap();
        track2.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        track2.set_stage_status(make_status_completed(ProcessingStage::Keyfinder));
        track2.set_stage_status(make_status_completed(ProcessingStage::Beatport));
        track2.set_stage_status(make_status_completed(ProcessingStage::Ai));

        // Now batch should be complete - track1 failed, track2 succeeded
        assert!(batch.is_complete());
    }
}
