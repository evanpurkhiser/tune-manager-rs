use std::collections::HashMap;

use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;
use thiserror::Error;

lazy_static! {
    static ref TRACK_PATH: Regex = Regex::new(r"^/track/[^/]+/(?<track_id>\d+)$").unwrap();
}

/// Extracts the track ID from a Beatport URL. Invalid beatport URLS will return None.
pub fn try_extract_track_id(maybe_beatport_url: &str) -> Option<u32> {
    maybe_beatport_url
        .parse::<reqwest::Url>()
        .ok()
        .filter(|url| {
            url.host_str()
                .unwrap_or("")
                .split('.')
                .collect::<Vec<_>>()
                .ends_with(&["beatport", "com"])
        })
        .and_then(|url| {
            TRACK_PATH
                .captures(url.path())?
                .name("track_id")?
                .as_str()
                .parse()
                .ok()
        })
}

#[derive(Error, Debug)]
pub enum BeatportApiError {
    #[error("Failed to make request to beatport.com")]
    RequestError(#[from] reqwest::Error),

    #[error("Missing release ID for track")]
    MissingRelaseId,

    #[error("Missing authorization. No token provided?")]
    NeedsAuth,
}

#[derive(Debug, PartialEq)]
pub struct BeatportTrackInfo {
    catalog_number: Option<String>,
    label: Option<String>,
    track_number: Option<u64>,
    track_total: Option<u64>,
    genre: Option<String>,
}

pub struct BeatportSource {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,

    /// When provided will transalte the genre returned from beatport to the mapped genre.
    genre_translation: Option<HashMap<String, String>>,
}

impl Default for BeatportSource {
    fn default() -> Self {
        let client = reqwest::ClientBuilder::new()
            .cookie_store(true)
            .build()
            .unwrap();

        Self {
            client,
            base_url: "https://beatport.com".to_string(),
            token: None,
            genre_translation: None,
        }
    }
}

impl BeatportSource {
    pub fn new(token: String) -> Self {
        Self {
            token: Some(token),
            ..Default::default()
        }
    }

    pub async fn fetch_track_info(
        &self,
        track_id: u32,
    ) -> Result<BeatportTrackInfo, BeatportApiError> {
        let token = self.token.as_ref().ok_or(BeatportApiError::NeedsAuth)?;

        let url = format!("{}/v4/catalog/tracks/{}", self.base_url, track_id);
        let track_resp = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let track: Value = track_resp.json().await?;

        let release_id = track["release"]["id"]
            .as_u64()
            .ok_or(BeatportApiError::MissingRelaseId)?;

        let url = format!("{}/v4/catalog/releases/{}", self.base_url, release_id);
        let release_resp = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let release: Value = release_resp.json().await?;

        let label = track["release"]["label"]["name"]
            .as_str()
            .map(str::to_string);
        let catalog_number = track["catalog_number"].as_str().map(str::to_string);
        let track_number = track["number"].as_u64();
        let track_total = release["track_count"].as_u64();
        let genre = track["genre"]["name"].as_str().map(str::to_string);

        Ok(BeatportTrackInfo {
            catalog_number,
            label,
            track_number,
            track_total,
            genre,
        })
    }
}

#[cfg(test)]
mod tests {
    use httpmock::prelude::*;
    use pretty_assertions::assert_eq;

    use super::BeatportSource;
    use crate::{
        beatport::{try_extract_track_id, BeatportTrackInfo},
        tests::read_fixture,
    };

    #[test]
    fn test_extract_track_id() {
        fn assert_url(url: &str, expected: Option<u32>) {
            assert_eq!(try_extract_track_id(url), expected);
        }

        assert_url("Invalid", None);
        assert_url("https://www.beatport.com/release/move/4600182", None);
        assert_url("https://blah.com/track/move-feat-malachiii/19119572", None);
        assert_url(
            "https://beatport.com/track/move-feat-malachiii/19119572",
            Some(19119572),
        );
        assert_url(
            "https://www.beatport.com/track/move-feat-malachiii/19119572",
            Some(19119572),
        );
    }

    #[tokio::test]
    async fn test_fetch_track_info() {
        let server = MockServer::start();

        let token = "1d7AICC9GxVcJsi1VnKrYJGfVeRShD";

        server.mock(|when, then| {
            when.method(GET)
                .path("/v4/catalog/tracks/1234")
                .header("Authorization", format!("Bearer {}", token));
            then.status(200)
                .header("content-type", "application/json")
                .body(read_fixture("beatport_track.json"));
        });
        server.mock(|when, then| {
            when.method(GET)
                .path("/v4/catalog/releases/439354")
                .header("Authorization", format!("Bearer {}", token));
            then.status(200)
                .header("content-type", "application/json")
                .body(read_fixture("beatport_release.json"));
        });

        let source = BeatportSource {
            token: Some(token.to_string()),
            base_url: server.base_url(),
            ..Default::default()
        };

        let info = source.fetch_track_info(1234).await.unwrap();

        assert_eq!(
            info,
            BeatportTrackInfo {
                catalog_number: Some("247HC031".to_string()),
                label: Some("24/7 Hardcore".to_string()),
                track_number: Some(1),
                track_total: Some(2),
                genre: Some("Techno (Peak Time / Driving)".to_string()),
            }
        );
    }
}
