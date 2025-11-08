use std::{io, path::PathBuf};

use id3::Tag;
use tracing::{error, info};

use crate::services::beatport::{
    BeatportCredentials, BeatportSource, try_extract_track_id, try_extract_url,
};

pub fn run(username: String, password: String, file_path: Option<PathBuf>) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_async(username, password, file_path))
}

async fn run_async(
    username: String,
    password: String,
    file_path: Option<PathBuf>,
) -> io::Result<()> {
    // Authenticate first
    let credentials = BeatportCredentials { username, password };
    let beatport_source = BeatportSource::new()
        .authenticate(credentials)
        .await
        .map_err(|e| {
            error!("Beatport authentication failed: {}", e);
            io::Error::new(io::ErrorKind::PermissionDenied, e)
        })?;

    info!("Beatport authentication successful!");

    let Some(path) = file_path else {
        info!("Token: {}", beatport_source.token());
        return Ok(());
    };

    // Read tags from file
    let tag = Tag::read_from_path(&path).map_err(|e| {
        error!("Failed to read tags from {}: {}", path.display(), e);
        io::Error::new(io::ErrorKind::InvalidData, e)
    })?;

    // Extract Beatport URL from WOAF frame
    let beatport_url = try_extract_url(&tag).ok_or_else(|| {
        error!("No Beatport URL found in WOAF frame");
        io::Error::new(io::ErrorKind::NotFound, "No Beatport URL found")
    })?;

    info!("Found Beatport URL: {}", beatport_url);

    // Parse track ID
    let track_id = try_extract_track_id(&beatport_url).ok_or_else(|| {
        error!(
            "Could not extract track ID from Beatport URL: {}",
            beatport_url
        );
        io::Error::new(io::ErrorKind::InvalidData, "Invalid Beatport URL")
    })?;

    info!("Extracted track ID: {}", track_id);

    // Fetch track info using the fixed beatport module
    info!("Fetching track info for ID: {}", track_id);
    let track_info = beatport_source
        .fetch_track_info(track_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch track info for ID {}: {}", track_id, e);
            io::Error::other(e)
        })?;

    println!("{:#?}", track_info);

    Ok(())
}
