use std::{path::PathBuf, sync::Arc};

use tokio::sync::mpsc;

use crate::processing::{
    concurrent::{ItemStatus, SentItemLike},
    coordinator::batch::{BatchId, BatchStageInput, TrackStageStatus},
    stages::{StageInput, StageStatus, ai, beatport, keyfinder, prepare_media},
};

/// Dispatch a single piece of stage work to the appropriate processor
pub fn handle_stage_dispatch(
    batch_stage_input: BatchStageInput,
    prepare_media_sender: &prepare_media::PrepareMediaSender,
    keyfinder_sender: &keyfinder::KeyfinderSender,
    beatport_sender: &beatport::BeatportSender,
    ai_sender: &ai::AiSender,
    status_update_sender: &mpsc::UnboundedSender<TrackStageStatus>,
) {
    let BatchStageInput {
        batch_id,
        stage_input,
    } = batch_stage_input;
    match stage_input {
        StageInput::PrepareMedia(input) => {
            let status_tx = status_update_sender.clone();
            let sender = prepare_media_sender.clone();
            let file_path = input.file_path.clone();

            tokio::spawn(async move {
                let sent_item = sender.send(input);
                monitor_stage_completion(vec![file_path], batch_id, sent_item, status_tx).await;
            });
        }
        StageInput::Keyfinder(input) => {
            let status_tx = status_update_sender.clone();
            let sender = keyfinder_sender.clone();
            let file_path = input.file_path.clone();

            tokio::spawn(async move {
                let sent_item = sender.send(input);
                monitor_stage_completion(vec![file_path], batch_id, sent_item, status_tx).await;
            });
        }
        StageInput::Beatport(input) => {
            let status_tx = status_update_sender.clone();
            let sender = beatport_sender.clone();
            let file_path = input.file_path.clone();

            tokio::spawn(async move {
                let sent_item = sender.send(input);
                monitor_stage_completion(vec![file_path], batch_id, sent_item, status_tx).await;
            });
        }
        StageInput::Ai(input) => {
            let status_tx = status_update_sender.clone();
            let sender = ai_sender.clone();

            let file_paths: Vec<_> = input
                .tracks
                .iter()
                .map(|track| track.metadata.file_path.clone())
                .collect();

            tokio::spawn(async move {
                let sent_item = sender.send(input);
                monitor_stage_completion(file_paths, batch_id, sent_item, status_tx).await;
            });
        }
    }
}

async fn monitor_stage_completion<T, P, Output, Error>(
    file_paths: P,
    batch_id: BatchId,
    mut sent_item: T,
    status_tx: mpsc::UnboundedSender<TrackStageStatus>,
) where
    T: SentItemLike<SentItemStatus = ItemStatus<Output, Error>>,
    T::SentItemStatus: Into<StageStatus>,
    P: IntoIterator<Item = PathBuf>,
{
    let file_paths: Vec<_> = file_paths.into_iter().collect();

    while let Some(status) = sent_item.next_status().await {
        let is_done = matches!(&status, ItemStatus::Complete(_) | ItemStatus::Skipped(_));

        let status = Arc::new(status.into());
        for file_path in &file_paths {
            let track_status = TrackStageStatus {
                batch_id: batch_id.clone(),
                file_path: file_path.clone(),
                status: status.clone(),
            };
            status_tx.send(track_status).unwrap();
        }

        // TODO: Do we actually need to check this? the sent item should get dropped once the
        // processor completes?
        if is_done {
            break;
        }
    }
}
