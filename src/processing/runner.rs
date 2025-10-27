use std::{io, path::PathBuf};

use tracing::info;

use crate::{app::config::Config, file_utils, processing::coordinator::ProcessingCoordinator};

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

    // Process all files as a single batch
    let batch_handle = coordinator.process_batch(files);

    // Wait for this batch to complete
    batch_handle
        .await_completion()
        .await
        .expect("Failed to wait for batch completion");

    info!("Processing completed");
    Ok(())
}
