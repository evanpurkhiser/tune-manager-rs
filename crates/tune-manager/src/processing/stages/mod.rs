use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use crate::processing::{concurrent::ItemStatus, state::TrackRevision};

pub mod ai;
pub mod beatport;
pub mod keyfinder;
pub mod lint_fix;
pub mod prepare_media;

/// Trait for types that can produce a TrackRevision from their results
pub trait ProducesRevision {
    /// Produces a revision, optionally using a previous revision for context
    fn produce_revision(&self, last_revision: Option<&TrackRevision>) -> Option<TrackRevision>;
}

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

    /// Runs the linter engine and applies auto-fixes. Remaining violations are
    /// preserved on the result for downstream stages (e.g. AI) to consume.
    LintFix,

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
            ProcessingStage::LintFix => StageMode::IndividualTrack,
            ProcessingStage::Ai => StageMode::Batch,
        }
    }

    /// Produce a StageStatus marked as skipped with the given reason for this processing stage.
    pub fn as_skipped_status(&self, reason: String) -> StageStatus {
        match self {
            ProcessingStage::PrepareMedia => StageStatus::PrepareMedia(ItemStatus::Skipped(reason)),
            ProcessingStage::Keyfinder => StageStatus::Keyfinder(ItemStatus::Skipped(reason)),
            ProcessingStage::Beatport => StageStatus::Beatport(ItemStatus::Skipped(reason)),
            ProcessingStage::LintFix => StageStatus::LintFix(ItemStatus::Skipped(reason)),
            ProcessingStage::Ai => StageStatus::Ai(ItemStatus::Skipped(reason)),
        }
    }
}

#[derive(Debug)]
pub enum StageInput {
    PrepareMedia(prepare_media::PrepareMediaInput),
    Keyfinder(keyfinder::KeyfinderInput),
    Beatport(beatport::BeatportInput),
    LintFix(lint_fix::LintFixInput),
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

impl From<lint_fix::LintFixInput> for StageInput {
    fn from(value: lint_fix::LintFixInput) -> Self {
        StageInput::LintFix(value)
    }
}

impl From<ai::AiInput> for StageInput {
    fn from(value: ai::AiInput) -> Self {
        StageInput::Ai(value)
    }
}

impl StageInput {
    /// Get the ProcessingStage that this status represents.
    pub fn stage(&self) -> ProcessingStage {
        match self {
            Self::PrepareMedia(_) => ProcessingStage::PrepareMedia,
            Self::Keyfinder(_) => ProcessingStage::Keyfinder,
            Self::Beatport(_) => ProcessingStage::Beatport,
            Self::LintFix(_) => ProcessingStage::LintFix,
            Self::Ai(_) => ProcessingStage::Ai,
        }
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum StageStatus {
    PrepareMedia(prepare_media::ItemStatus),
    Keyfinder(keyfinder::ItemStatus),
    Beatport(beatport::ItemStatus),
    LintFix(lint_fix::ItemStatus),
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

impl From<lint_fix::ItemStatus> for StageStatus {
    fn from(value: lint_fix::ItemStatus) -> Self {
        StageStatus::LintFix(value)
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
            StageStatus::LintFix(_) => ProcessingStage::LintFix,
            StageStatus::Ai(_) => ProcessingStage::Ai,
        }
    }

    pub fn item_status(&self) -> ItemStatus<(), ()> {
        match self {
            StageStatus::PrepareMedia(status) => extract_status!(status),
            StageStatus::Keyfinder(status) => extract_status!(status),
            StageStatus::Beatport(status) => extract_status!(status),
            StageStatus::LintFix(status) => extract_status!(status),
            StageStatus::Ai(status) => extract_status!(status),
        }
    }

    /// Check if this stage has completed or was skipped. This does not guarantee that the Result
    /// of the [`ItemStatus::Complete`] is Ok.
    pub fn is_done(&self) -> bool {
        let status = self.item_status();
        matches!(status, ItemStatus::Complete(_) | ItemStatus::Skipped(_))
    }

    /// Check if this stage completed successfully
    pub fn is_success(&self) -> bool {
        let status = self.item_status();
        matches!(status, ItemStatus::Complete(Ok(_)))
    }

    /// Check if this stage has failed (completed with an error)
    pub fn has_failed(&self) -> bool {
        let status = self.item_status();
        matches!(status, ItemStatus::Complete(Err(_)))
    }

    /// Check if this stage was skipped
    pub fn is_skipped(&self) -> bool {
        let status = self.item_status();
        matches!(status, ItemStatus::Skipped(_))
    }

    /// Produces an Option that implements [`ProducesRevision`]. Will produce None if the stage
    /// status is not [`ItemStatus::Complete`].
    fn as_successful_result(&self) -> Option<&dyn ProducesRevision> {
        match self {
            StageStatus::PrepareMedia(ItemStatus::Complete(Ok(result))) => Some(result),
            StageStatus::Keyfinder(ItemStatus::Complete(Ok(result))) => Some(result),
            StageStatus::Beatport(ItemStatus::Complete(Ok(result))) => Some(result),
            StageStatus::LintFix(ItemStatus::Complete(Ok(result))) => Some(result),
            StageStatus::Ai(ItemStatus::Complete(Ok(result))) => Some(result),
            StageStatus::PrepareMedia(_)
            | StageStatus::Keyfinder(_)
            | StageStatus::Beatport(_)
            | StageStatus::LintFix(_)
            | StageStatus::Ai(_) => None,
        }
    }
}

impl ProducesRevision for StageStatus {
    fn produce_revision(&self, last_revision: Option<&TrackRevision>) -> Option<TrackRevision> {
        self.as_successful_result()?.produce_revision(last_revision)
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use id3::Tag;
    use std::{collections::HashMap, path::PathBuf, sync::Arc};

    /// Create a mock StageStatus with Running status
    pub fn make_status_running(stage: ProcessingStage) -> Arc<StageStatus> {
        let status = match stage {
            ProcessingStage::PrepareMedia => StageStatus::PrepareMedia(ItemStatus::Running),
            ProcessingStage::Keyfinder => StageStatus::Keyfinder(ItemStatus::Running),
            ProcessingStage::Beatport => StageStatus::Beatport(ItemStatus::Running),
            ProcessingStage::LintFix => StageStatus::LintFix(ItemStatus::Running),
            ProcessingStage::Ai => StageStatus::Ai(ItemStatus::Running),
        };
        Arc::new(status)
    }

    /// Create a mock StageStatus with Complete status
    pub fn make_status_completed(stage: ProcessingStage) -> Arc<StageStatus> {
        let status = match stage {
            ProcessingStage::PrepareMedia => {
                let result = prepare_media::PrepareMediaResult {
                    file_path: PathBuf::from("/test/file.aiff"),
                    media_hash: vec![0u8; 16],
                    tag: Tag::new(),
                };
                StageStatus::PrepareMedia(ItemStatus::Complete(Ok(result)))
            }
            ProcessingStage::Keyfinder => {
                let result = keyfinder::KeyfinderResult {
                    detected_key: Some("10A".to_string()),
                };
                StageStatus::Keyfinder(ItemStatus::Complete(Ok(result)))
            }
            ProcessingStage::Beatport => {
                StageStatus::Beatport(ItemStatus::Skipped("No Beatport URL".to_string()))
            }
            ProcessingStage::LintFix => {
                let result = lint_fix::LintFixResult {
                    fields: Default::default(),
                    results: Vec::new(),
                    hit_max_iterations: false,
                };
                StageStatus::LintFix(ItemStatus::Complete(Ok(result)))
            }
            ProcessingStage::Ai => {
                let result = ai::AiResult {
                    responses: HashMap::new(),
                };
                StageStatus::Ai(ItemStatus::Complete(Ok(result)))
            }
        };
        Arc::new(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use id3::{Tag, TagLike};
    use std::path::PathBuf;

    #[test]
    fn test_produce_revision_from_prepare_media() {
        let mut tag = Tag::new();
        tag.set_title("Test Track");

        let result = prepare_media::PrepareMediaResult {
            file_path: PathBuf::from("/test/track.mp3"),
            tag: tag.clone(),
            media_hash: vec![1, 2, 3],
        };

        // PrepareMedia doesn't need previous revision
        let status = StageStatus::PrepareMedia(ItemStatus::Complete(Ok(result)));
        let revision = status.produce_revision(None);

        assert!(revision.is_some());
    }

    #[test]
    fn test_produce_revision_from_keyfinder_with_previous() {
        let prev_revision = TrackRevision::new(Default::default());

        let result = keyfinder::KeyfinderResult {
            detected_key: Some("Am".to_string()),
        };

        let status = StageStatus::Keyfinder(ItemStatus::Complete(Ok(result)));
        let revision = status.produce_revision(Some(&prev_revision));

        assert!(revision.is_some());
    }

    #[test]
    fn test_produce_revision_from_keyfinder_without_previous() {
        let result = keyfinder::KeyfinderResult {
            detected_key: Some("Am".to_string()),
        };

        let status = StageStatus::Keyfinder(ItemStatus::Complete(Ok(result)));
        let revision = status.produce_revision(None);

        // Should return None because Keyfinder needs previous revision
        assert!(revision.is_none());
    }

    #[test]
    fn test_produce_revision_from_failed_stage() {
        use prepare_media::{ContainerError, PrepareMediaError};

        let error = PrepareMediaError::Container(ContainerError::BadPath);
        let status = StageStatus::PrepareMedia(ItemStatus::Complete(Err(error)));
        let revision = status.produce_revision(None);

        // Failed stages should not produce revisions
        assert!(revision.is_none());
    }

    #[test]
    fn test_produce_revision_from_running_stage() {
        let status = StageStatus::PrepareMedia(ItemStatus::Running);
        let revision = status.produce_revision(None);

        // Running stages should not produce revisions
        assert!(revision.is_none());
    }
}
