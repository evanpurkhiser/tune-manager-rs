use std::io;

use id3::Tag;
use tracing::info;

use crate::{
    app::config::BeatportConfig,
    beatport::{self, BeatportCredentials, BeatportSource, BeatportTrackInfo, try_extract_url},
};

#[derive(Debug)]
pub struct BeatportResult {
    pub track_info: Option<BeatportTrackInfo>,
}

pub async fn run(
    tag: &Tag,
    beatport_config: Option<&BeatportConfig>,
) -> io::Result<BeatportResult> {
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
