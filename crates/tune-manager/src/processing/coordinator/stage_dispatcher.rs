use std::{collections::HashMap, path::PathBuf};

use strum::IntoEnumIterator;
use tokio::sync::mpsc;

use crate::{
    processing::{
        stages::{ProcessingStage, StageInput, StageMode, ai, beatport, keyfinder, prepare_media},
        state,
    },
    track::Track,
};

use super::batch::{BatchId, BatchStageInput, ProcessingBatch, TrackProcessingState};

/// Dispatch next eligible stages for all tracks in a batch
pub fn dispatch_next_stages(
    batch: &mut ProcessingBatch,
    stage_dispatch_tx: &mpsc::UnboundedSender<BatchStageInput>,
) {
    // Dispatch individual track stages that are ready
    ProcessingStage::iter()
        .filter(|stage| stage.mode() == StageMode::IndividualTrack)
        .for_each(|stage| {
            batch
                .tracks
                .values_mut()
                .filter(|track| track.can_run_stage(&stage))
                .filter_map(|track| {
                    create_stage_input(&stage, track)
                        .map(|stage_input| BatchStageInput {
                            batch_id: batch.id.clone(),
                            stage_input,
                        })
                        .map(|batch_input| (batch_input, track))
                })
                .for_each(|(batch_input, track)| {
                    stage_dispatch_tx.send(batch_input).unwrap();
                    track.stage_dispatched.insert(stage.clone());
                });
        });

    // Handle AI batch processing at batch level
    let has_dispatched_ai = batch.stage_dispatched.contains(&ProcessingStage::Ai);

    if !has_dispatched_ai && all_tracks_ready_for_ai(&batch.tracks) {
        dispatch_ai_batch(&batch.tracks, stage_dispatch_tx, &batch.id);
        batch.stage_dispatched.insert(ProcessingStage::Ai);
    }
}

/// Check if all non-failed tracks are ready for AI processing
fn all_tracks_ready_for_ai(tracks: &HashMap<PathBuf, TrackProcessingState>) -> bool {
    tracks
        .values()
        .filter(|track| !track.has_failed_stage())
        .all(|track| track.can_run_stage(&ProcessingStage::Ai))
}

/// Create the appropriate StageInput for a given stage and track
fn create_stage_input(
    stage: &ProcessingStage,
    track_state: &TrackProcessingState,
) -> Option<StageInput> {
    match stage {
        ProcessingStage::PrepareMedia => {
            let file_path = track_state.file_path.clone();
            Some(prepare_media::PrepareMediaInput { file_path }.into())
        }
        ProcessingStage::Keyfinder => {
            let file_path = track_state.file_path.clone();
            Some(keyfinder::KeyfinderInput { file_path }.into())
        }
        ProcessingStage::Beatport => {
            let file_path = track_state.file_path.clone();
            track_state
                .tag
                .clone()
                .map(|tag| beatport::BeatportInput { file_path, tag }.into())
        }
        ProcessingStage::Ai => None,
    }
}

/// Dispatch AI processing for all eligible tracks as a batch
fn dispatch_ai_batch(
    tracks: &HashMap<PathBuf, TrackProcessingState>,
    stage_dispatch_tx: &mpsc::UnboundedSender<BatchStageInput>,
    batch_id: &BatchId,
) {
    let ai_tracks: Vec<_> = tracks
        .values()
        .filter_map(|track| {
            track
                .tag
                .as_ref()
                .and_then(state::get_last_revision)
                .map(|revision| Track {
                    metadata: track.file_path.clone().into(),
                    tags: revision.tags,
                })
        })
        .collect();

    if !ai_tracks.is_empty() {
        let ai_input = ai::AiInput { tracks: ai_tracks };
        let batch_stage_input = BatchStageInput {
            batch_id: batch_id.clone(),
            stage_input: ai_input.into(),
        };
        stage_dispatch_tx.send(batch_stage_input).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use id3::Tag;
    use tokio::sync::oneshot;

    use super::*;
    use crate::{
        processing::stages::{ProcessingStage, test_helpers::*},
        track::TrackRevision,
    };

    // Helper to create a batch with a single track
    fn make_batch_with_track(file_path: PathBuf) -> ProcessingBatch {
        let (tx, _rx) = oneshot::channel();
        ProcessingBatch::new(vec![file_path], tx)
    }

    // Helper to mark a stage as complete and dispatched
    fn mark_stage_complete(track: &mut TrackProcessingState, stage: ProcessingStage) {
        track.set_stage_status(make_status_completed(stage.clone()));
        track.stage_dispatched.insert(stage);
    }

    #[test]
    fn test_dispatch_prepare_media_for_new_track() {
        let file_path = PathBuf::from("/test/track.mp3");
        let mut batch = make_batch_with_track(file_path.clone());
        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();

        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should dispatch PrepareMedia
        let dispatched = stage_dispatch_rx.try_recv().unwrap();
        assert_eq!(dispatched.batch_id, batch.id);
        assert_eq!(
            dispatched.stage_input.stage(),
            ProcessingStage::PrepareMedia
        );

        // Track should be marked as dispatched
        let track = batch.tracks.get(&file_path).unwrap();
        assert!(
            track
                .stage_dispatched
                .contains(&ProcessingStage::PrepareMedia)
        );

        // Should be no more dispatches
        assert!(stage_dispatch_rx.try_recv().is_err());
    }

    #[test]
    fn test_dispatch_after_prepare_media() {
        let file_path = PathBuf::from("/test/track.mp3");
        let mut batch = make_batch_with_track(file_path.clone());
        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();

        // Mark PrepareMedia as complete
        let track = batch.tracks.get_mut(&file_path).unwrap();
        mark_stage_complete(track, ProcessingStage::PrepareMedia);
        track.tag = Some(Tag::new());

        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should dispatch both Keyfinder and Beatport (they run in parallel)
        let mut dispatched_stages = vec![];
        while let Ok(dispatched) = stage_dispatch_rx.try_recv() {
            dispatched_stages.push(dispatched.stage_input.stage());
        }

        assert_eq!(dispatched_stages.len(), 2);
        assert!(dispatched_stages.contains(&ProcessingStage::Keyfinder));
        assert!(dispatched_stages.contains(&ProcessingStage::Beatport));
    }

    #[test]
    fn test_no_dispatch_if_already_dispatched() {
        let file_path = PathBuf::from("/test/track.mp3");
        let mut batch = make_batch_with_track(file_path.clone());
        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();

        let track = batch.tracks.get_mut(&file_path).unwrap();

        // Mark PrepareMedia as complete (so downstream stages could run)
        mark_stage_complete(track, ProcessingStage::PrepareMedia);

        // Mark other stages as dispatched, but not complete
        track.stage_dispatched.insert(ProcessingStage::Keyfinder);
        track.stage_dispatched.insert(ProcessingStage::Beatport);
        track.tag = Some(Tag::new());

        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should not dispatch anything because all stages are already marked as dispatched
        assert!(stage_dispatch_rx.try_recv().is_err());
    }

    #[test]
    fn test_no_dispatch_if_prerequisites_not_met() {
        let file_path = PathBuf::from("/test/track.mp3");
        let mut batch = make_batch_with_track(file_path.clone());
        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();

        // Mark PrepareMedia as dispatched but NOT complete
        // This tests that Keyfinder and Beatport won't dispatch when prerequisites aren't met
        let track = batch.tracks.get_mut(&file_path).unwrap();
        track.stage_dispatched.insert(ProcessingStage::PrepareMedia);

        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should not dispatch anything because:
        // - PrepareMedia: already in stage_dispatched
        // - Keyfinder: can_run_stage returns false (PrepareMedia not done)
        // - Beatport: can_run_stage returns false (PrepareMedia not done)
        assert!(stage_dispatch_rx.try_recv().is_err());
    }

    #[test]
    fn test_dispatch_ai_when_all_tracks_ready() {
        // Create a temporary file so TrackMetadaata can read its mtime
        let temp_file = tempfile::Builder::new().suffix(".mp3").tempfile().unwrap();
        let file_path = temp_file.path().to_path_buf();

        let mut batch = make_batch_with_track(file_path.clone());
        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();

        // Complete all prerequisite stages
        let track = batch.tracks.get_mut(&file_path).unwrap();
        mark_stage_complete(track, ProcessingStage::PrepareMedia);
        mark_stage_complete(track, ProcessingStage::Keyfinder);
        mark_stage_complete(track, ProcessingStage::Beatport);

        // Need to add a tag with revision for AI to work
        let mut tag = Tag::new();
        let revision = TrackRevision::new(Default::default());
        state::append_track_revision(&mut tag, revision).unwrap();
        track.tag = Some(tag);

        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should dispatch AI
        let dispatched = stage_dispatch_rx.try_recv().unwrap();
        assert_eq!(dispatched.batch_id, batch.id);
        assert_eq!(dispatched.stage_input.stage(), ProcessingStage::Ai);

        // Batch should be marked as AI dispatched
        assert!(batch.stage_dispatched.contains(&ProcessingStage::Ai));
    }

    #[test]
    fn test_no_ai_dispatch_if_already_dispatched() {
        let file_path = PathBuf::from("/test/track.mp3");
        let mut batch = make_batch_with_track(file_path.clone());
        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();

        // Complete all stages
        let track = batch.tracks.get_mut(&file_path).unwrap();
        mark_stage_complete(track, ProcessingStage::PrepareMedia);
        mark_stage_complete(track, ProcessingStage::Keyfinder);
        mark_stage_complete(track, ProcessingStage::Beatport);
        track.tag = Some(Tag::new());

        // Mark AI as already dispatched at batch level
        batch.stage_dispatched.insert(ProcessingStage::Ai);

        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should not dispatch anything
        assert!(stage_dispatch_rx.try_recv().is_err());
    }

    #[test]
    fn test_dispatch_with_multiple_tracks() {
        let file1 = PathBuf::from("/test/track1.mp3");
        let file2 = PathBuf::from("/test/track2.mp3");
        let (tx, _rx) = oneshot::channel();
        let mut batch = ProcessingBatch::new(vec![file1.clone(), file2.clone()], tx);
        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();

        // Track 1: PrepareMedia complete, ready for next stages
        let track1 = batch.tracks.get_mut(&file1).unwrap();
        mark_stage_complete(track1, ProcessingStage::PrepareMedia);
        track1.tag = Some(Tag::new());

        // Track 2: Nothing dispatched yet
        // (PrepareMedia will be dispatched)

        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should dispatch:
        // - PrepareMedia for track2
        // - Keyfinder for track1
        // - Beatport for track1
        let mut dispatched_stages = vec![];
        while let Ok(dispatched) = stage_dispatch_rx.try_recv() {
            dispatched_stages.push(dispatched.stage_input.stage());
        }

        assert_eq!(dispatched_stages.len(), 3);
        assert_eq!(
            dispatched_stages
                .iter()
                .filter(|s| **s == ProcessingStage::PrepareMedia)
                .count(),
            1
        );
        assert!(dispatched_stages.contains(&ProcessingStage::Keyfinder));
        assert!(dispatched_stages.contains(&ProcessingStage::Beatport));
    }

    #[test]
    fn test_ai_waits_for_all_tracks_before_dispatching() {
        // Create temporary files so TrackMetadata can read mtime
        let temp_file1 = tempfile::Builder::new().suffix(".mp3").tempfile().unwrap();
        let file1 = temp_file1.path().to_path_buf();

        let temp_file2 = tempfile::Builder::new().suffix(".mp3").tempfile().unwrap();
        let file2 = temp_file2.path().to_path_buf();

        let (tx, _rx) = oneshot::channel();
        let mut batch = ProcessingBatch::new(vec![file1.clone(), file2.clone()], tx);
        let (stage_dispatch_tx, mut stage_dispatch_rx) = mpsc::unbounded_channel();

        // Track 1: Complete all stages up to and including Beatport (ready for AI)
        let track1 = batch.tracks.get_mut(&file1).unwrap();
        mark_stage_complete(track1, ProcessingStage::PrepareMedia);
        mark_stage_complete(track1, ProcessingStage::Keyfinder);
        mark_stage_complete(track1, ProcessingStage::Beatport);
        let mut tag1 = Tag::new();
        let revision1 = TrackRevision::new(Default::default());
        state::append_track_revision(&mut tag1, revision1).unwrap();
        track1.tag = Some(tag1);

        // Track 2: Only complete PrepareMedia (NOT ready for AI yet)
        let track2 = batch.tracks.get_mut(&file2).unwrap();
        mark_stage_complete(track2, ProcessingStage::PrepareMedia);
        track2.tag = Some(Tag::new());

        // First dispatch: track1 is ready for AI, but track2 is not
        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should dispatch Keyfinder and Beatport for track2, but NOT AI
        let mut dispatched_stages = vec![];
        while let Ok(dispatched) = stage_dispatch_rx.try_recv() {
            dispatched_stages.push(dispatched.stage_input.stage());
        }

        assert_eq!(dispatched_stages.len(), 2);
        assert!(dispatched_stages.contains(&ProcessingStage::Keyfinder));
        assert!(dispatched_stages.contains(&ProcessingStage::Beatport));
        assert!(!dispatched_stages.contains(&ProcessingStage::Ai));
        assert!(!batch.stage_dispatched.contains(&ProcessingStage::Ai));

        // Now complete track2's Keyfinder and Beatport stages
        let track2 = batch.tracks.get_mut(&file2).unwrap();
        mark_stage_complete(track2, ProcessingStage::Keyfinder);
        mark_stage_complete(track2, ProcessingStage::Beatport);
        let mut tag2 = Tag::new();
        let revision2 = TrackRevision::new(Default::default());
        state::append_track_revision(&mut tag2, revision2).unwrap();
        track2.tag = Some(tag2);

        // Second dispatch: both tracks are now ready for AI
        dispatch_next_stages(&mut batch, &stage_dispatch_tx);

        // Should dispatch AI batch with both tracks
        let dispatched = stage_dispatch_rx.try_recv().unwrap();
        assert_eq!(dispatched.batch_id, batch.id);
        assert_eq!(dispatched.stage_input.stage(), ProcessingStage::Ai);

        // Verify it's an AI batch with 2 tracks
        match dispatched.stage_input {
            StageInput::Ai(ai_input) => {
                assert_eq!(ai_input.tracks.len(), 2);
            }
            _ => panic!("Expected AI stage input"),
        }

        // Batch should now be marked as AI dispatched
        assert!(batch.stage_dispatched.contains(&ProcessingStage::Ai));

        // No more dispatches
        assert!(stage_dispatch_rx.try_recv().is_err());
    }

    #[test]
    fn test_all_tracks_ready_for_ai_excludes_failed_tracks() {
        use crate::processing::{
            concurrent::ItemStatus,
            stages::{StageStatus, prepare_media},
        };

        let mut tracks = HashMap::new();

        // Track 1: Ready for AI (all prerequisites complete)
        let mut track1 = TrackProcessingState::new("/test/track1.mp3");
        track1.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        track1.set_stage_status(make_status_completed(ProcessingStage::Keyfinder));
        track1.set_stage_status(make_status_completed(ProcessingStage::Beatport));
        tracks.insert(PathBuf::from("/test/track1.mp3"), track1);

        // Track 2: Failed at PrepareMedia
        let mut track2 = TrackProcessingState::new("/test/track2.mp3");
        let error =
            prepare_media::PrepareMediaError::Container(prepare_media::ContainerError::BadPath);
        let failed_status = StageStatus::PrepareMedia(ItemStatus::Complete(Err(error)));
        track2.set_stage_status(Arc::new(failed_status));
        tracks.insert(PathBuf::from("/test/track2.mp3"), track2);

        // Should be ready for AI because track2 is excluded (failed)
        // and track1 is ready
        assert!(all_tracks_ready_for_ai(&tracks));
    }

    #[test]
    fn test_all_tracks_ready_for_ai_waits_for_non_failed_tracks() {
        let mut tracks = HashMap::new();

        // Track 1: Ready for AI
        let mut track1 = TrackProcessingState::new("/test/track1.mp3");
        track1.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        track1.set_stage_status(make_status_completed(ProcessingStage::Keyfinder));
        track1.set_stage_status(make_status_completed(ProcessingStage::Beatport));
        tracks.insert(PathBuf::from("/test/track1.mp3"), track1);

        // Track 2: Still processing (not failed, just not ready yet)
        let mut track2 = TrackProcessingState::new("/test/track2.mp3");
        track2.set_stage_status(make_status_completed(ProcessingStage::PrepareMedia));
        tracks.insert(PathBuf::from("/test/track2.mp3"), track2);

        // Should NOT be ready for AI because track2 hasn't completed Beatport yet
        assert!(!all_tracks_ready_for_ai(&tracks));
    }
}
