use std::{
    io,
    path::{Path, PathBuf},
};

use tracing::info;

use crate::{
    app::config::{BeatportConfig, Config},
    beatport::{self, BeatportCredentials, BeatportSource, BeatportTrackInfo, try_extract_url},
    keyfinder,
};
use id3::Tag;

use super::stages;

pub async fn process_file(file_path: PathBuf, config: &Config) -> io::Result<()> {
    info!("Starting processing pipeline for: {}", file_path.display());

    // Stage 1: PrepareMedia
    info!("=== Stage 1: PrepareMedia ===");
    let prepare_result = run_prepare_media(&file_path).inspect_err(|e| {
        eprintln!("PrepareMedia failed: {}", e);
    })?;
    info!("PrepareMedia completed successfully");

    // Stage 2: Keyfinder
    info!("=== Stage 2: Keyfinder ===");
    let keyfinder_result = run_keyfinder(&prepare_result.file_path).inspect_err(|e| {
        eprintln!("Keyfinder failed: {}", e);
    })?;
    info!("Keyfinder completed successfully");
    println!("Keyfinder Result: {:#?}", keyfinder_result);

    // Stage 3: Beatport
    info!("=== Stage 3: Beatport ===");
    let beatport_result = run_beatport(&prepare_result.tag, config.beatport.as_ref())
        .await
        .inspect_err(|e| {
            eprintln!("Beatport failed: {}", e);
        })?;
    info!("Beatport completed successfully");
    println!("Beatport Result: {:#?}", beatport_result);

    info!(
        "Processing pipeline completed for: {}",
        prepare_result.file_path.display()
    );
    Ok(())
}

// Use the result struct from the stages module
use stages::prepare_media::PrepareMediaResult;

#[derive(Debug)]
struct KeyfinderResult {
    detected_key: Option<String>,
    notation: String,
}

#[derive(Debug)]
struct BeatportResult {
    url_found: bool,
    track_info: Option<BeatportTrackInfo>,
    api_success: bool,
}

fn run_prepare_media(file_path: &Path) -> io::Result<PrepareMediaResult> {
    match stages::prepare_media::run(file_path) {
        Ok(result) => Ok(result),
        Err(e) => Err(io::Error::new(
            io::ErrorKind::Other,
            format!("PrepareMedia failed: {}", e),
        )),
    }
}

fn run_keyfinder(file_path: &Path) -> io::Result<KeyfinderResult> {
    info!("Running Keyfinder stage on: {}", file_path.display());

    match keyfinder::detect_key(file_path, keyfinder::KeyNotation::Standard) {
        Ok(key_result) => {
            info!("Keyfinder detection completed");
            Ok(KeyfinderResult {
                detected_key: key_result,
                notation: "Standard".to_string(),
            })
        }
        Err(e) => Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Keyfinder failed: {}", e),
        )),
    }
}

async fn run_beatport(
    tag: &Tag,
    beatport_config: Option<&BeatportConfig>,
) -> io::Result<BeatportResult> {
    info!("Running Beatport stage");

    // Look for WOAF frame with Beatport URL
    let Some(url) = try_extract_url(tag) else {
        info!("No Beatport URL found in WOAF frame");
        return Ok(BeatportResult {
            url_found: false,
            track_info: None,
            api_success: false,
        });
    };

    info!("Found Beatport URL: {}", url);

    // Try to extract track ID
    let Some(track_id) = beatport::try_extract_track_id(&url) else {
        info!("Could not extract track ID from Beatport URL");
        return Ok(BeatportResult {
            url_found: false,
            track_info: None,
            api_success: false,
        });
    };

    info!("Extracted Beatport track ID: {}", track_id);

    // If we have beatport credentials, try to fetch track info
    let Some(config) = beatport_config else {
        info!("No Beatport credentials configured, skipping API call");
        return Ok(BeatportResult {
            url_found: true,
            track_info: None,
            api_success: false,
        });
    };

    info!("Authenticating with Beatport and fetching track info");

    let credentials = BeatportCredentials {
        username: config.username.clone(),
        password: config.password.clone(),
    };

    let Ok(authenticated_source) = BeatportSource::new().authenticate(credentials).await else {
        return Ok(BeatportResult {
            url_found: true,
            track_info: None,
            api_success: false,
        });
    };

    let Ok(track_info) = authenticated_source.fetch_track_info(track_id).await else {
        return Ok(BeatportResult {
            url_found: true,
            track_info: None,
            api_success: false,
        });
    };

    info!("Successfully fetched track info from Beatport API");
    Ok(BeatportResult {
        url_found: true,
        track_info: Some(track_info),
        api_success: true,
    })
}
