use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use crate::processing::concurrent::ItemStatus;

pub mod ai;
pub mod beatport;
pub mod keyfinder;
pub mod prepare_media;

/// Represents the different stages of the processing pipeline
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Display, EnumString, EnumIter, Serialize, Deserialize,
)]
pub enum ProcessingStage {
    /// Converts media to supported format (AIFF/MP3), ensures ID3v2.4 tags, and computes media hash
    PrepareMedia,

    /// Detects the musical key of the audio using keyfinder-cli
    Keyfinder,

    /// Extracts Beatport URL from WOAF tags and fetches track metadata from Beatport API
    Beatport,

    /// Uses AI to clean up and normalize track metadata
    Ai,
}

/// Represents the level at which a stage operates, either on individual tracks, or on the batch as
/// a whole
#[derive(Debug, PartialEq)]
pub enum StageMode {
    Batch,
    IndividualTrack,
}

impl ProcessingStage {
    /// Returns the mode in which this stage operates
    pub fn mode(&self) -> StageMode {
        match self {
            ProcessingStage::PrepareMedia => StageMode::IndividualTrack,
            ProcessingStage::Keyfinder => StageMode::IndividualTrack,
            ProcessingStage::Beatport => StageMode::IndividualTrack,
            ProcessingStage::Ai => StageMode::Batch,
        }
    }
}

#[derive(Debug)]
pub enum StageInput {
    PrepareMedia(prepare_media::PrepareMediaInput),
    Keyfinder(keyfinder::KeyfinderInput),
    Beatport(beatport::BeatportInput),
    Ai(ai::AiInput),
}

impl From<prepare_media::PrepareMediaInput> for StageInput {
    fn from(value: prepare_media::PrepareMediaInput) -> Self {
        StageInput::PrepareMedia(value)
    }
}

impl From<keyfinder::KeyfinderInput> for StageInput {
    fn from(value: keyfinder::KeyfinderInput) -> Self {
        StageInput::Keyfinder(value)
    }
}

impl From<beatport::BeatportInput> for StageInput {
    fn from(value: beatport::BeatportInput) -> Self {
        StageInput::Beatport(value)
    }
}

impl From<ai::AiInput> for StageInput {
    fn from(value: ai::AiInput) -> Self {
        StageInput::Ai(value)
    }
}

#[derive(Debug)]
pub enum StageStatus {
    PrepareMedia(prepare_media::ItemStatus),
    Keyfinder(keyfinder::ItemStatus),
    Beatport(beatport::ItemStatus),
    Ai(ai::ItemStatus),
}

impl From<prepare_media::ItemStatus> for StageStatus {
    fn from(value: prepare_media::ItemStatus) -> Self {
        StageStatus::PrepareMedia(value)
    }
}

impl From<keyfinder::ItemStatus> for StageStatus {
    fn from(value: keyfinder::ItemStatus) -> Self {
        StageStatus::Keyfinder(value)
    }
}

impl From<beatport::ItemStatus> for StageStatus {
    fn from(value: beatport::ItemStatus) -> Self {
        StageStatus::Beatport(value)
    }
}

impl From<ai::ItemStatus> for StageStatus {
    fn from(value: ai::ItemStatus) -> Self {
        StageStatus::Ai(value)
    }
}

// Hash and Eq implementations based only on stage type, not status content.
// This allows HashSet to treat two StageStatus instances as equal if they
// represent the same stage.
impl std::hash::Hash for StageStatus {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.stage().hash(state);
    }
}

impl PartialEq for StageStatus {
    fn eq(&self, other: &Self) -> bool {
        self.stage() == other.stage()
    }
}

impl Eq for StageStatus {}

macro_rules! extract_status {
    ($status:expr) => {
        match $status {
            ItemStatus::Waiting => ItemStatus::Waiting,
            ItemStatus::Running => ItemStatus::Running,
            ItemStatus::Skipped(reason) => ItemStatus::Skipped(reason.clone()),
            ItemStatus::Complete(Ok(_)) => ItemStatus::Complete(Ok(())),
            ItemStatus::Complete(Err(_)) => ItemStatus::Complete(Err(())),
        }
    };
}

impl StageStatus {
    /// Get the ProcessingStage that this status represents.
    pub fn stage(&self) -> ProcessingStage {
        match self {
            StageStatus::PrepareMedia(_) => ProcessingStage::PrepareMedia,
            StageStatus::Keyfinder(_) => ProcessingStage::Keyfinder,
            StageStatus::Beatport(_) => ProcessingStage::Beatport,
            StageStatus::Ai(_) => ProcessingStage::Ai,
        }
    }

    pub fn item_status(&self) -> ItemStatus<(), ()> {
        match self {
            StageStatus::PrepareMedia(status) => extract_status!(status),
            StageStatus::Keyfinder(status) => extract_status!(status),
            StageStatus::Beatport(status) => extract_status!(status),
            StageStatus::Ai(status) => extract_status!(status),
        }
    }

    /// Check if this stage has completed or was skipped. This does not guarantee that the Result
    /// of the [`ItemStatus::Complete`] is Ok.
    pub fn is_done(&self) -> bool {
        let status = self.item_status();
        matches!(status, ItemStatus::Complete(_) | ItemStatus::Skipped(_))
    }
}
