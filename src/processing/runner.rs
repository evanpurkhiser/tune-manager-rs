use std::{
    io,
    path::{Path, PathBuf},
};

use tracing::info;

use crate::{
    app::config::Config,
    processing::{
        ProcessingStage,
        state::{append_track_revision, completed_stages, get_last_revision, mark_stage_complete},
    },
    track::{Track, TrackRevision, TrackTags},
};

use super::stages;
use stages::prepare_media::PrepareMediaResult;

pub async fn process_file(input_path: PathBuf, config: &Config) -> io::Result<()> {
    info!("Starting processing pipeline for: {}", input_path.display());

    info!("=== Stage 1: PrepareMedia ===");
    let prepare_result = run_prepare_media(&input_path).inspect_err(|e| {
        eprintln!("PrepareMedia failed: {}", e);
    })?;

    let mut tag = prepare_result.tag;
    let file_path = prepare_result.file_path;

    let complete = completed_stages(&tag);

    if !complete.contains(&ProcessingStage::PrepareMedia) {
        let revision = TrackRevision::new(TrackTags::from(&tag));
        append_track_revision(&mut tag, &revision).unwrap();
        mark_stage_complete(&mut tag, ProcessingStage::PrepareMedia).unwrap();

        info!("PrepareMedia completed successfully");
    } else {
        info!("PrepareMedia post processing skipped");
    }

    info!("=== Stage 2: Keyfinder ===");
    if !complete.contains(&ProcessingStage::Keyfinder) {
        let keyfinder_result = stages::keyfinder::run(&file_path).inspect_err(|e| {
            eprintln!("Keyfinder failed: {}", e);
        })?;

        if let Some(detected_key) = keyfinder_result.detected_key {
            let mut revision = get_last_revision(&tag).expect("to have revisions").clone();
            revision.tags.key = Some(detected_key.clone());
            append_track_revision(&mut tag, &revision).unwrap();
            mark_stage_complete(&mut tag, ProcessingStage::Keyfinder).unwrap();

            info!("Keyfinder completed: {:?}", detected_key);
        }
    } else {
        info!("KeyFinder stage skipped");
    }

    info!("=== Stage 3: Beatport ===");
    if !complete.contains(&ProcessingStage::Beatport) {
        let beatport_result = stages::beatport::run(&tag, config.beatport.as_ref())
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

    info!("=== Stage 4: AI ===");
    if !complete.contains(&ProcessingStage::Ai) {
        let revision = get_last_revision(&tag).expect("to have revisions").clone();

        let track = Track {
            metadata: file_path.clone().into(),
            tags: revision.tags.clone(),
        };

        let ai_result = stages::ai::run(track).await.inspect_err(|e| {
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

    info!("Processing pipeline completed for: {}", file_path.display());

    let revision = get_last_revision(&tag).expect("to have revisions").clone();
    println!("{}", serde_json::to_string_pretty(&revision).unwrap());

    tag.write_to_path(file_path, id3::Version::Id3v24).unwrap();

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
