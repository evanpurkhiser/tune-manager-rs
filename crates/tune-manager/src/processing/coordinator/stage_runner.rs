use std::{path::PathBuf, sync::Arc};

use tokio::sync::mpsc;

use crate::processing::{
    concurrent::{ItemStatus, SentItemLike},
    stages::{StageInput, StageStatus},
};

use super::{
    StageProcessors,
    batch::{BatchId, BatchStageInput},
};

/// Status update for a specific track's stage
#[derive(Debug)]
pub struct TrackStageStatus {
    pub batch_id: BatchId,
    pub file_path: PathBuf,
    pub status: Arc<StageStatus>,
}

/// Dispatch a single piece of stage work to the appropriate processor
pub fn handle_stage_dispatch(
    batch_stage_input: BatchStageInput,
    processors: &StageProcessors,
    status_update_sender: &mpsc::UnboundedSender<TrackStageStatus>,
) {
    let BatchStageInput {
        batch_id,
        stage_input,
    } = batch_stage_input;
    match stage_input {
        StageInput::PrepareMedia(input) => {
            let status_tx = status_update_sender.clone();
            let sender = processors.prepare_media_sender.clone();
            let file_path = input.file_path.clone();

            tokio::spawn(async move {
                let sent_item = sender.send(input);
                monitor_stage_completion(vec![file_path], batch_id, sent_item, status_tx).await;
            });
        }
        StageInput::Keyfinder(input) => {
            let status_tx = status_update_sender.clone();
            let sender = processors.keyfinder_sender.clone();
            let file_path = input.file_path.clone();

            tokio::spawn(async move {
                let sent_item = sender.send(input);
                monitor_stage_completion(vec![file_path], batch_id, sent_item, status_tx).await;
            });
        }
        StageInput::Beatport(input) => {
            let status_tx = status_update_sender.clone();
            let sender = processors.beatport_sender.clone();
            let file_path = input.file_path.clone();

            tokio::spawn(async move {
                let sent_item = sender.send(input);
                monitor_stage_completion(vec![file_path], batch_id, sent_item, status_tx).await;
            });
        }
        StageInput::Ai(input) => {
            let status_tx = status_update_sender.clone();
            let sender = processors.ai_sender.clone();

            let file_paths: Vec<_> = input
                .tracks
                .iter()
                .map(|track| track.file.file_path.clone())
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
        let status = Arc::new(status.into());
        for file_path in &file_paths {
            let track_status = TrackStageStatus {
                batch_id: batch_id.clone(),
                file_path: file_path.clone(),
                status: status.clone(),
            };
            status_tx.send(track_status).unwrap();
        }
    }
}
