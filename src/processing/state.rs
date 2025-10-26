use std::{borrow::Borrow, collections::HashSet};

use id3::{Tag, TagLike, frame};
use serde::{Deserialize, Serialize};

use super::stages::ProcessingStage;
use crate::track::TrackRevision;

const GEOB_STAGE_FILENAME: &str = "tune-manager-processing-state.json";
const GEOB_REVISIONS_FILENAME: &str = "tune-manager-track-revisions.json";

/// Represents the processing state stored in the GEOB frame
#[derive(Debug, Serialize, Deserialize, Default)]
struct ProcessingState {
    completed_stages: HashSet<ProcessingStage>,
}

/// Represents the track history stored in the GEOB frame
#[derive(Debug, Serialize, Deserialize, Default)]
struct TrackHistory {
    revisions: Vec<TrackRevision>,
}

/// Gets the completed processing stages from the tag's GEOB frame
pub fn completed_stages(tag: &Tag) -> HashSet<ProcessingStage> {
    tag.encapsulated_objects()
        .find(|geob| geob.filename == GEOB_STAGE_FILENAME)
        .and_then(|geob| serde_json::from_slice::<ProcessingState>(&geob.data).ok())
        .map(|state| state.completed_stages)
        .unwrap_or_default()
}

/// Marks a processing stage as complete in the tag's GEOB frame
pub fn mark_stage_complete(tag: &mut Tag, stage: ProcessingStage) -> Result<(), id3::Error> {
    let mut completed_stages = completed_stages(tag);
    completed_stages.insert(stage);

    let state = ProcessingState { completed_stages };

    let json_data = serde_json::to_vec(&state).map_err(|e| {
        let io_err = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Failed to serialize processing state",
        );
        id3::Error::new(id3::ErrorKind::Io(io_err), e.to_string())
    })?;

    // Remove existing GEOB frame with our filename
    tag.remove_encapsulated_object(None, None, Some(GEOB_STAGE_FILENAME), None);

    let geob = frame::EncapsulatedObject {
        mime_type: "application/json".to_string(),
        filename: GEOB_STAGE_FILENAME.to_string(),
        description: "Processing state for tune-manager".to_string(),
        data: json_data,
    };

    tag.add_frame(geob);
    Ok(())
}

/// Gets the track revisions history from the tag's GEOB frame
pub fn track_revisions(tag: &Tag) -> Vec<TrackRevision> {
    tag.encapsulated_objects()
        .find(|geob| geob.filename == GEOB_REVISIONS_FILENAME)
        .and_then(|geob| serde_json::from_slice::<TrackHistory>(&geob.data).ok())
        .map(|history| history.revisions)
        .unwrap_or_default()
}

/// Gets the most recent track revision from the tag's GEOB frame
pub fn get_last_revision(tag: &Tag) -> Option<TrackRevision> {
    track_revisions(tag).last().cloned()
}

/// Appends a track revision to the tag's GEOB frame
pub fn append_track_revision(
    tag: &mut Tag,
    revision: impl Borrow<TrackRevision>,
) -> Result<(), id3::Error> {
    let mut revisions = track_revisions(tag);
    revisions.push(revision.borrow().clone());

    let history = TrackHistory { revisions };

    let json_data = serde_json::to_vec(&history).map_err(|e| {
        let io_err = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Failed to serialize track revisions",
        );
        id3::Error::new(id3::ErrorKind::Io(io_err), e.to_string())
    })?;

    // Remove existing GEOB frame with our filename
    tag.remove_encapsulated_object(None, None, Some(GEOB_REVISIONS_FILENAME), None);

    let geob = frame::EncapsulatedObject {
        mime_type: "application/json".to_string(),
        filename: GEOB_REVISIONS_FILENAME.to_string(),
        description: "Track revisions history for tune-manager".to_string(),
        data: json_data,
    };

    tag.add_frame(geob);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::TrackTags;
    use id3::Tag;

    #[test]
    fn test_empty_tag_has_no_completed_stages() {
        let tag = Tag::new();
        let stages = completed_stages(&tag);
        assert!(stages.is_empty());
    }

    #[test]
    fn test_mark_and_get_completed_stage() {
        let mut tag = Tag::new();

        // Mark PrepareMedia as complete
        mark_stage_complete(&mut tag, ProcessingStage::PrepareMedia).unwrap();

        let stages = completed_stages(&tag);
        assert_eq!(stages, [ProcessingStage::PrepareMedia].into());
    }

    #[test]
    fn test_mark_multiple_stages() {
        let mut tag = Tag::new();

        // Mark stages as complete one by one
        mark_stage_complete(&mut tag, ProcessingStage::PrepareMedia).unwrap();
        mark_stage_complete(&mut tag, ProcessingStage::Keyfinder).unwrap();

        let stages = completed_stages(&tag);
        assert_eq!(
            stages,
            [ProcessingStage::PrepareMedia, ProcessingStage::Keyfinder].into()
        );
    }

    #[test]
    fn test_empty_tag_has_no_track_revisions() {
        let tag = Tag::new();
        let revisions = track_revisions(&tag);
        assert!(revisions.is_empty());
    }

    #[test]
    fn test_append_track_revision() {
        let mut tag = Tag::new();

        let tags = TrackTags {
            artist: Some("Test Artist".to_string()),
            title: Some("Test Title".to_string()),
            ..Default::default()
        };

        let revision = TrackRevision::new(tags.clone());

        // Append first revision
        append_track_revision(&mut tag, revision).unwrap();

        let revisions = track_revisions(&tag);
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].tags.artist, Some("Test Artist".to_string()));
        assert_eq!(revisions[0].tags.title, Some("Test Title".to_string()));
    }

    #[test]
    fn test_append_multiple_track_revisions() {
        let mut tag = Tag::new();

        let tags1 = TrackTags {
            artist: Some("Artist 1".to_string()),
            title: Some("Title 1".to_string()),
            album: None,
            ..Default::default()
        };

        let tags2 = TrackTags {
            artist: Some("Artist 2".to_string()),
            title: Some("Title 2".to_string()),
            album: Some("Album 2".to_string()),
            ..Default::default()
        };

        let revision1 = TrackRevision::new(tags1);
        let revision2 = TrackRevision::new(tags2);

        // Append revisions
        append_track_revision(&mut tag, revision1).unwrap();
        append_track_revision(&mut tag, revision2).unwrap();

        let revisions = track_revisions(&tag);
        assert_eq!(revisions.len(), 2);

        // First revision
        assert_eq!(revisions[0].tags.artist, Some("Artist 1".to_string()));
        assert_eq!(revisions[0].tags.title, Some("Title 1".to_string()));
        assert_eq!(revisions[0].tags.album, None);

        // Second revision
        assert_eq!(revisions[1].tags.artist, Some("Artist 2".to_string()));
        assert_eq!(revisions[1].tags.title, Some("Title 2".to_string()));
        assert_eq!(revisions[1].tags.album, Some("Album 2".to_string()));

        // Check timestamps are ordered (second should be after first)
        assert!(revisions[1].ts >= revisions[0].ts);
    }
}
