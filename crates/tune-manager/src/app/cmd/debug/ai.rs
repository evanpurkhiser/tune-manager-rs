use std::{io, path::PathBuf};

use async_openai::Client;
use tracing::{error, info, warn};

use crate::{
    file_utils,
    services::ai,
    track::{TaggedFile, Track},
};

pub fn run(path: PathBuf) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_async(path));

    Ok(())
}

async fn run_async(path: PathBuf) {
    let mut problematic_files = vec![];

    let tracks: Vec<Track> = file_utils::walk_music_files(&path)
        .filter_map(|entry| match TaggedFile::read(entry.path().to_owned()) {
            Ok(file) => Some(file),
            Err(_) => {
                problematic_files.push(entry);
                None
            }
        })
        .map(Track::from)
        .collect();

    if !problematic_files.is_empty() {
        warn!("Found {} problematic files", problematic_files.len());
        for file in &problematic_files {
            warn!("  - {}", file.path().display());
        }
    }

    let client = Client::new();

    info!("Processing {} tracks with AI...", tracks.len());
    match ai::process_tracks(&client, tracks).await {
        Ok(result) => {
            info!("AI processing successful!");
            info!("{:#?}", result);
        }
        Err(e) => {
            error!("AI processing failed: {}", e);
        }
    }
}
