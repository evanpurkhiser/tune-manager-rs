use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use id3::Tag;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::processing::stages::{ProcessingStage, StageInput, StageStatus, prepare_media};

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

/// Status update for a specific track's stage
#[derive(Debug)]
pub struct TrackStageStatus {
    pub batch_id: BatchId,
    pub file_path: PathBuf,
    pub status: Arc<StageStatus>,
}

/// Represents a batch of tracks being processed together
#[derive(Debug)]
pub struct ProcessingBatch {
    pub id: BatchId,

    /// Stages that have been dispatched for this batch
    pub stage_dispatched: HashSet<ProcessingStage>,

    /// Map of file paths to their processing state
    pub tracks: HashMap<PathBuf, TrackProcessingState>,
}

impl ProcessingBatch {
    /// Create a new ProcessingBatch with the given files
    pub fn new(files: Vec<PathBuf>) -> Self {
        let mut tracks = HashMap::new();
        for file_path in files {
            let track_state = TrackProcessingState::new(file_path.clone());
            tracks.insert(file_path, track_state);
        }

        Self {
            id: BatchId::new(),
            stage_dispatched: HashSet::new(),
            tracks,
        }
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

    /// Check if all prerequisite stages for a given stage are complete and the stage has not
    /// already been dispatched
    pub fn can_run_stage(&self, stage: &ProcessingStage) -> bool {
        let stage_ready = match stage {
            ProcessingStage::PrepareMedia => true,
            ProcessingStage::Keyfinder => self.is_stage_done(ProcessingStage::PrepareMedia),
            ProcessingStage::Beatport => self.is_stage_done(ProcessingStage::PrepareMedia),
            ProcessingStage::Ai => self.is_stage_done(ProcessingStage::Beatport),
        };

        stage_ready && !self.stage_dispatched.contains(stage)
    }
}

/// Stores and dispatches a new ProcessingBatch
pub fn handle_new_batch(
    batches: &mut HashMap<BatchId, ProcessingBatch>,
    stage_dispatch_tx: &mpsc::UnboundedSender<BatchStageInput>,
    batch: ProcessingBatch,
) {
    let batch_id = batch.id.clone();

    // Collect PrepareMediaInput
    let inputs: Vec<BatchStageInput> = batch
        .tracks
        .keys()
        .map(|file_path| {
            let batch_id = batch_id.clone();
            let file_path = file_path.clone();
            let prepare_media_input = prepare_media::PrepareMediaInput { file_path };

            BatchStageInput {
                batch_id,
                stage_input: prepare_media_input.into(),
            }
        })
        .collect();

    // Register the batch and dispatch the PrepareMedia stage for all tracks
    batches.insert(batch_id, batch);
    inputs
        .into_iter()
        .for_each(|i| stage_dispatch_tx.send(i).unwrap());
}
