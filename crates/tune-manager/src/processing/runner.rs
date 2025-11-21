use std::{io, path::PathBuf};

use tracing::info;

use crate::{
    app::config::Config,
    file_utils,
    processing::coordinator::{batch::{BatchConfig, StatusEvent}, callbacks::callback, ProcessingCoordinator},
};

pub async fn process_path(path: PathBuf, config: &Config) -> io::Result<()> {
    let files = if path.is_file() {
        vec![path]
    } else if path.is_dir() {
        let files: Vec<PathBuf> = file_utils::walk_music_files(&path)
            .map(|entry| entry.path().to_path_buf())
            .collect();

        if files.is_empty() {
            info!("No files found in directory: {}", path.display());
            return Ok(());
        }
        files
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Path does not exist: {}", path.display()),
        ));
    };

    let coordinator = ProcessingCoordinator::start(config);

    // Register logging callback
    let _log_handle = coordinator.on_status(callback(|event| match event {
        StatusEvent::TrackStageUpdate {
            file_path,
            status,
            revision,
            ..
        } => {
            info!(
                "Status update for {}: {:?} - {:?}",
                file_path.display(),
                status.stage(),
                status.item_status()
            );

            if let Some(rev) = revision.as_ref() {
                info!(
                    "Revision added for {} after {:?}: {}",
                    file_path.display(),
                    status.stage(),
                    serde_json::to_string_pretty(rev)
                        .unwrap_or_else(|_| "Failed to serialize".to_string())
                );
            }
        }
        StatusEvent::BatchCompleted { batch } => {
            info!("Batch {} completed", batch.id);
        }
    }));

    // Process all files as a single batch
    let config = BatchConfig::default();
    let batch_handle = coordinator.process_batch(files, config);

    // Wait for this batch to complete
    batch_handle
        .await_completion()
        .await
        .expect("Failed to wait for batch completion");

    info!("Processing completed");
    Ok(())
}
