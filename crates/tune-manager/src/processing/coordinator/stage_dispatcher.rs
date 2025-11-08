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
    if !batch.stage_dispatched.contains(&ProcessingStage::Ai) {
        let all_tracks_ready_for_ai = batch
            .tracks
            .values()
            .all(|track| track.can_run_stage(&ProcessingStage::Ai));

        if all_tracks_ready_for_ai {
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
