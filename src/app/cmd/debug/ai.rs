use std::{io, path::PathBuf};

use async_openai::Client;
use id3::Tag;
use tracing::{error, info, warn};

use crate::{ai, file_utils, track::Track};

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
        .map(|e| (e.to_owned(), Tag::read_from_path(e.path())))
        .filter_map(|(entry, tag)| match tag {
            Ok(t) => Some((entry.path().to_owned(), t)),
            Err(_) => {
                problematic_files.push(entry);
                None
            }
        })
        .map(|(entry, tag)| Track::from((entry, tag)))
        .collect();

    if !problematic_files.is_empty() {
        warn!("Found {} problematic files", problematic_files.len());
        for file in &problematic_files {
            warn!("  - {}", file.path().display());
        }
    }

    let client = Client::new();

    info!("Processing {} tracks with AI...", tracks.len());
    match ai::process_tracks(client, tracks).await {
        Ok(result) => {
            info!("AI processing successful!");
            info!("{:#?}", result);
        }
        Err(e) => {
            error!("AI processing failed: {}", e);
        }
    }
}
