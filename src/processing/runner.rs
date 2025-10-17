use std::{io, path::PathBuf};

use tracing::info;

use crate::{
    app::config::Config,
    processing::{
        ProcessingStage,
        stages::{
            ai::{AiError, AiInput, new_ai_processor},
            beatport::{BeatportError, BeatportInput, new_beatport_processor},
            keyfinder::{KeyfinderInput, new_keyfinder_processor},
            prepare_media::{PrepareMediaInput, new_prepare_media_processor},
        },
        state::{append_track_revision, completed_stages, get_last_revision, mark_stage_complete},
    },
    track::{Track, TrackRevision, TrackTags},
};

pub async fn process_file(input_path: PathBuf, config: &Config) -> io::Result<()> {
    info!("Starting processing pipeline for: {}", input_path.display());

    // Create processors
    let prepare_media_processor = new_prepare_media_processor();
    let prepare_media_sender = prepare_media_processor.get_sender();

    let keyfinder_processor = new_keyfinder_processor();
    let keyfinder_sender = keyfinder_processor.get_sender();

    let beatport_processor = new_beatport_processor(config.beatport.as_ref());
    let beatport_sender = beatport_processor.get_sender();

    let ai_processor = new_ai_processor(config.ai.as_ref());
    let ai_sender = ai_processor.get_sender();

    // Start processors in background
    tokio::spawn(prepare_media_processor.start());
    tokio::spawn(keyfinder_processor.start());
    tokio::spawn(beatport_processor.start());
    tokio::spawn(ai_processor.start());

    info!("=== Stage 1: PrepareMedia ===");
    let prepare_input = PrepareMediaInput {
        file_path: input_path.clone(),
    };
    let prepare_result = prepare_media_sender
        .send(prepare_input)
        .result()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("PrepareMedia failed: {}", e)))?;

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
        let keyfinder_input = KeyfinderInput {
            file_path: file_path.clone(),
        };
        let keyfinder_result = keyfinder_sender
            .send(keyfinder_input)
            .result()
            .await
            .map_err(|e| {
                io::Error::new(io::ErrorKind::Other, format!("Keyfinder failed: {}", e))
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
        let beatport_input = BeatportInput { tag: tag.clone() };
        match beatport_sender.send(beatport_input).result().await {
            Ok(beatport_result) => {
                let mut revision = get_last_revision(&tag).expect("to have revisions").clone();
                if let Some(ref track_info) = beatport_result.track_info {
                    track_info.update_track_tags(&mut revision.tags);
                }
                append_track_revision(&mut tag, &revision).unwrap();
                mark_stage_complete(&mut tag, ProcessingStage::Beatport).unwrap();

                info!("Beatport completed successfully");
                println!("Beatport Result: {:#?}", beatport_result);
            }
            Err(BeatportError::NotConfigured) => {
                info!("No Beatport credentials configured, skipping stage");
                // Continue without error - just mark stage complete without updating tags
                let revision = get_last_revision(&tag).expect("to have revisions").clone();
                append_track_revision(&mut tag, &revision).unwrap();
                mark_stage_complete(&mut tag, ProcessingStage::Beatport).unwrap();
            }
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Beatport failed: {}", e),
                ));
            }
        }
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

        let ai_input = AiInput {
            tracks: vec![track],
        };
        match ai_sender.send(ai_input).result().await {
            Ok(ai_result) => {
                if let Some(track_response) = ai_result.responses.first() {
                    let mut revision = get_last_revision(&tag).expect("to have revisions").clone();
                    track_response.update_track_tags(&mut revision.tags);
                    append_track_revision(&mut tag, &revision).unwrap();
                    mark_stage_complete(&mut tag, ProcessingStage::Ai).unwrap();
                    info!("AI completed successfully");
                } else {
                    // Mark stage complete even if no AI response
                    mark_stage_complete(&mut tag, ProcessingStage::Ai).unwrap();
                    info!("AI processing completed with no response");
                }
            }
            Err(AiError::NotConfigured) => {
                info!("OpenAI not configured, skipping AI stage");
                // Continue without error - just mark stage complete without updating tags
                mark_stage_complete(&mut tag, ProcessingStage::Ai).unwrap();
            }
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("AI failed: {}", e),
                ));
            }
        }
    } else {
        info!("AI stage skipped");
    }

    info!("Processing pipeline completed for: {}", file_path.display());

    let revision = get_last_revision(&tag).expect("to have revisions").clone();
    println!("{}", serde_json::to_string_pretty(&revision).unwrap());

    //tag.write_to_path(file_path, id3::Version::Id3v24).unwrap();

    Ok(())
}
