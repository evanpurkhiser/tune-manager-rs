use std::{collections::HashMap, mem::replace, sync::Arc};

use tokio::sync::mpsc;
use tracing::error;

use crate::{
    processing::{
        concurrent::ItemStatus,
        stages::{StageStatus, ai, beatport, keyfinder, prepare_media},
        state,
    },
    track::TrackRevision,
};

use super::{
    batch::{BatchId, BatchStageInput, BatchState, ProcessingBatch, StatusEvent, TrackStageStatus},
    callbacks::CallbackRegistry,
    stage_dispatcher::dispatch_next_stages,
};

/// Handle a status update from a processor
pub fn handle_track_status(
    batches: &mut HashMap<BatchId, ProcessingBatch>,
    stage_dispatch_tx: &mpsc::UnboundedSender<BatchStageInput>,
    callback_registry: &Arc<CallbackRegistry>,
    track_status: TrackStageStatus,
) {
    let TrackStageStatus {
        batch_id,
        file_path,
        status,
    } = track_status;

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
    if let Some(ref revision) = revision {
        track_state.tag.as_mut().map(|tag| {
            state::append_track_revision(tag, revision.clone()).expect("revision added");
            tag.write_to_path(file_path.clone(), id3::Version::Id3v24)
        });
    }

    track_state.set_stage_status(status.clone());

    batch.tracks.insert(track_key, track_state);

    // Emit track stage update event
    let track_update_event = StatusEvent::TrackStageUpdate {
        batch,
        file_path: file_path.clone(),
        status,
        revision: Box::new(revision),
    };
    callback_registry.invoke_all(&track_update_event);

    if dispatch_next_stage {
        dispatch_next_stages(batch, stage_dispatch_tx);
    }

    // Check if batch is complete and notify if so
    if batch.is_complete() {
        if let BatchState::Processing(completion_tx) =
            replace(&mut batch.state, BatchState::Complete)
        {
            let _ = completion_tx.send(());
        }

        // Emit batch completed event
        let batch_completed_event = StatusEvent::BatchCompleted { batch };
        callback_registry.invoke_all(&batch_completed_event);
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
