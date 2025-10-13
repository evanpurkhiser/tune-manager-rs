use std::io;

use crate::{ai, track::Track};

#[derive(Debug)]
pub struct AiResult {
    pub response: Option<ai::TrackResponse>,
}

pub async fn run(track: Track) -> io::Result<AiResult> {
    // Use AI to process the track
    let ai_client = async_openai::Client::new();
    let response = ai::process_tracks(ai_client, vec![track])
        .await
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("AI processing failed: {}", e),
            )
        })?;

    if let Some(track_response) = response.tracks.first() {
        let response = Some(track_response.clone());
        Ok(AiResult { response })
    } else {
        Ok(AiResult { response: None })
    }
}
