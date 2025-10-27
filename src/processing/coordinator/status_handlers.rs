use std::{collections::HashMap, mem::replace, path::PathBuf};

use tokio::sync::mpsc;
use tracing::{error, info};

use strum::IntoEnumIterator;

use crate::{
    processing::{
        concurrent::ItemStatus,
        stages::{
            ProcessingStage, StageInput, StageMode, StageStatus, ai, beatport, keyfinder,
            prepare_media,
        },
        state,
    },
    track::{Track, TrackRevision},
};

use super::batch::{
    BatchId, BatchStageInput, BatchState, ProcessingBatch, TrackProcessingState, TrackStageStatus,
};

/// Handle a status update from a processor
pub fn handle_track_status(
    batches: &mut HashMap<BatchId, ProcessingBatch>,
    stage_dispatch_tx: &mpsc::UnboundedSender<BatchStageInput>,
    track_status: TrackStageStatus,
) {
    let TrackStageStatus {
        batch_id,
        file_path,
        status,
    } = track_status;

    info!(
        "Status update for {}: {:?} - {:?}",
        file_path.display(),
        status.stage(),
        status.item_status()
    );

    let Some(batch) = batches.get_mut(&batch_id) else {
        error!("Received status for unknown batch: {}", batch_id);
        return;
    };

    let track_key = file_path.clone();
    let Some(mut track_state) = batch.tracks.remove(&track_key) else {
        error!(
            "Received status for unknown track: {} in batch {}",
            file_path.display(),
            batch_id
        );
        return;
    };

    if let StageStatus::PrepareMedia(ItemStatus::Complete(Ok(result))) = status.as_ref() {
        track_state.file_path = result.file_path.clone();
        track_state.tag = Some(result.tag.clone());
    }

    // Get previous revision for stages that need it
    let last_revision = track_state.tag.as_ref().and_then(state::get_last_revision);

    // Extract revision from the completed stage
    let revision = extract_revision_from_status(&status, last_revision.as_ref());

    // Only dispatch next stages if this stage completed successfully or was skipped
    let dispatch_next_stage = matches!(
        status.item_status(),
        ItemStatus::Complete(Ok(())) | ItemStatus::Skipped(_)
    );

    // Handle successful completion - update track tags with new revision
    if let Some(revision) = revision {
        track_state
            .tag
            .as_mut()
            .map(|tag| state::append_track_revision(tag, revision.clone()));

        info!(
            "Revision added for {} after {:?}: {}",
            file_path.display(),
            status.stage(),
            serde_json::to_string_pretty(&revision)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

        // TODO: We should write tags here
    }

    track_state.set_stage_status(status);

    batch.tracks.insert(track_key, track_state);

    if dispatch_next_stage {
        dispatch_next_stages(batch, stage_dispatch_tx);
    }

    // Check if batch is complete and notify if so
    if batch.is_complete() {
        info!("Batch {} completed", batch_id);
        if let BatchState::Processing(completion_tx) =
            replace(&mut batch.state, BatchState::Complete)
        {
            let _ = completion_tx.send(());
        }
    }
}

/// Extract a TrackRevision from a completed stage status, with previous revision for stages that need it
fn extract_revision_from_status(
    status: &StageStatus,
    last_revision: Option<&TrackRevision>,
) -> Option<TrackRevision> {
    match status {
        StageStatus::PrepareMedia(ItemStatus::Complete(Ok(result))) => {
            Some(prepare_media::produce_revision(&result.tag))
        }
        StageStatus::Keyfinder(ItemStatus::Complete(Ok(result))) => {
            last_revision.map(|prev| keyfinder::produce_revision(prev, result))
        }
        StageStatus::Beatport(ItemStatus::Complete(Ok(result))) => {
            last_revision.map(|prev| beatport::produce_revision(prev, result))
        }
        StageStatus::Ai(ItemStatus::Complete(Ok(result))) => {
            last_revision.and_then(|prev| ai::produce_revision(prev, result))
        }
        _ => None, // Not completed or failed
    }
}

/// Dispatch next eligible stages for all tracks in a batch
fn dispatch_next_stages(
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
    if !batch.stage_dispatched.contains(&ProcessingStage::Ai) {
        let all_tracks_ready_for_ai = batch
            .tracks
            .values()
            .all(|track| track.can_run_stage(&ProcessingStage::Ai));

        if all_tracks_ready_for_ai {
            info!(
                "All tracks in batch {} ready for AI - dispatching batch",
                batch.id
            );
            dispatch_ai_batch(&batch.tracks, stage_dispatch_tx, &batch.id);
            batch.stage_dispatched.insert(ProcessingStage::Ai);
        }
    }
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
