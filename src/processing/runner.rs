use std::{
    io,
    path::{Path, PathBuf},
};

use id3::Tag;
use tracing::info;

use crate::{
    ai,
    app::config::{BeatportConfig, Config},
    beatport::{self, BeatportCredentials, BeatportSource, BeatportTrackInfo, try_extract_url},
    keyfinder,
    processing::{
        ProcessingStage,
        state::{append_track_revision, completed_stages, get_last_revision, mark_stage_complete},
    },
    track::{Track, TrackRevision, TrackTags},
};

use super::stages;
use stages::prepare_media::PrepareMediaResult;

pub async fn process_file(file_path: PathBuf, config: &Config) -> io::Result<()> {
    info!("Starting processing pipeline for: {}", file_path.display());

    // Stage 1: PrepareMedia
    info!("=== Stage 1: PrepareMedia ===");
    let prepare_result = run_prepare_media(&file_path).inspect_err(|e| {
        eprintln!("PrepareMedia failed: {}", e);
    })?;

    let mut tag = prepare_result.tag;
    let complete = completed_stages(&tag);

    if !complete.contains(&ProcessingStage::PrepareMedia) {
        let revision = TrackRevision::new(TrackTags::from(&tag));
        append_track_revision(&mut tag, &revision).unwrap();
        mark_stage_complete(&mut tag, ProcessingStage::PrepareMedia).unwrap();

        info!("PrepareMedia completed successfully");
    } else {
        info!("PrepareMedia post processing skipped");
    }

    // Stage 2: Keyfinder
    info!("=== Stage 2: Keyfinder ===");
    if !complete.contains(&ProcessingStage::Keyfinder) {
        let keyfinder_result = run_keyfinder(&prepare_result.file_path).inspect_err(|e| {
            eprintln!("Keyfinder failed: {}", e);
        })?;

        let mut revision = get_last_revision(&tag).expect("to have revisions").clone();
        revision.tags.key = keyfinder_result.detected_key.clone();
        append_track_revision(&mut tag, &revision).unwrap();
        mark_stage_complete(&mut tag, ProcessingStage::Keyfinder).unwrap();

        info!(
            "Keyfinder completed: {:?}",
            keyfinder_result.detected_key.clone()
        );
    } else {
        info!("KeyFinder stage skipped");
    }

    // Stage 3: Beatport
    info!("=== Stage 3: Beatport ===");
    if !complete.contains(&ProcessingStage::Beatport) {
        let beatport_result = run_beatport(&tag, config.beatport.as_ref())
            .await
            .inspect_err(|e| {
                eprintln!("Beatport failed: {}", e);
            })?;

        let mut revision = get_last_revision(&tag).expect("to have revisions").clone();
        if let Some(ref track_info) = beatport_result.track_info {
            track_info.update_track_tags(&mut revision.tags);
        }
        append_track_revision(&mut tag, &revision).unwrap();
        mark_stage_complete(&mut tag, ProcessingStage::Beatport).unwrap();

        info!("Beatport completed successfully");
        println!("Beatport Result: {:#?}", beatport_result);
    } else {
        info!("Beatport stage skipped");
    }

    // Stage 4: AI
    info!("=== Stage 4: AI ===");
    if !complete.contains(&ProcessingStage::Ai) {
        let revision = get_last_revision(&tag).expect("to have revisions").clone();

        let ai_result = run_ai(&revision.tags, &prepare_result.file_path)
            .await
            .inspect_err(|e| {
                eprintln!("AI failed: {}", e);
            })?;

        if let Some(ref track_response) = ai_result.response {
            let mut revision = get_last_revision(&tag).expect("to have revisions").clone();
            track_response.update_track_tags(&mut revision.tags);
            append_track_revision(&mut tag, &revision).unwrap();
            mark_stage_complete(&mut tag, ProcessingStage::Ai).unwrap();
            info!("AI completed successfully");
        }
    } else {
        info!("AI stage skipped");
    }

    info!(
        "Processing pipeline completed for: {}",
        prepare_result.file_path.display()
    );

    let revision = get_last_revision(&tag).expect("to have revisions").clone();
    println!("{}", serde_json::to_string_pretty(&revision).unwrap());

    Ok(())
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

#[derive(Debug)]
struct KeyfinderResult {
    detected_key: Option<String>,
}

fn run_keyfinder(file_path: &Path) -> io::Result<KeyfinderResult> {
    match keyfinder::detect_key(file_path, keyfinder::KeyNotation::Camelot) {
        Ok(detected_key) => {
            info!("Keyfinder detection completed");
            Ok(KeyfinderResult { detected_key })
        }
        Err(e) => Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Keyfinder failed: {}", e),
        )),
    }
}

#[derive(Debug)]
struct BeatportResult {
    track_info: Option<BeatportTrackInfo>,
}

async fn run_beatport(
    tag: &Tag,
    beatport_config: Option<&BeatportConfig>,
) -> io::Result<BeatportResult> {
    info!("Running Beatport stage");

    // Look for WOAF frame with Beatport URL
    let Some(url) = try_extract_url(tag) else {
        info!("No Beatport URL found in WOAF frame");
        return Ok(BeatportResult { track_info: None });
    };

    info!("Found Beatport URL: {}", url);

    // Try to extract track ID
    let Some(track_id) = beatport::try_extract_track_id(&url) else {
        info!("Could not extract track ID from Beatport URL");
        return Ok(BeatportResult { track_info: None });
    };

    info!("Extracted Beatport track ID: {}", track_id);

    // If we have beatport credentials, try to fetch track info
    let Some(config) = beatport_config else {
        info!("No Beatport credentials configured, skipping API call");
        return Ok(BeatportResult { track_info: None });
    };

    info!("Authenticating with Beatport and fetching track info");

    let credentials = BeatportCredentials {
        username: config.username.clone(),
        password: config.password.clone(),
    };

    let Ok(authenticated_source) = BeatportSource::new().authenticate(credentials).await else {
        return Ok(BeatportResult { track_info: None });
    };

    let Ok(track_info) = authenticated_source.fetch_track_info(track_id).await else {
        return Ok(BeatportResult { track_info: None });
    };

    info!("Successfully fetched track info from Beatport API");
    Ok(BeatportResult {
        track_info: Some(track_info),
    })
}

#[derive(Debug)]
struct AiResult {
    response: Option<ai::TrackResponse>,
}

async fn run_ai(track_tags: &TrackTags, file_path: &Path) -> io::Result<AiResult> {
    info!("Running AI stage");

    // Create a Track from current state with actual file metadata
    let track = Track {
        metadata: file_path.to_path_buf().into(),
        tags: track_tags.clone(),
    };

    // Use AI to process the track
    let ai_client = async_openai::Client::new();
    let Ok(response) = ai::process_tracks(ai_client, vec![track]).await else {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "AI processing failed".to_string(),
        ));
    };

    if let Some(track_response) = response.tracks.first() {
        info!("AI processing completed successfully");
        Ok(AiResult {
            response: Some(track_response.clone()),
        })
    } else {
        info!("AI returned no track updates");
        Ok(AiResult { response: None })
    }
}
