use std::{borrow::Borrow, collections::HashMap, sync::LazyLock};

use id3::{Tag, TagLike};
use regex::Regex;
use serde_json::{Value, json};
use thiserror::Error;

use crate::{fields::Count, fields::CountField, track::TrackFields};

static TRACK_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/track/[^/]+/(?<track_id>\d+)$").unwrap());

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

/// Extracts the beatport URL from a Tag if present in the WOAF frame.
pub fn try_extract_url(tag: &Tag) -> Option<String> {
    tag.get("WOAF")
        .and_then(|frame| frame.content().link())
        .map(str::to_string)
        .filter(|url| url.contains("beatport.com"))
}

#[derive(Error, Debug)]
pub enum BeatportApiError {
    #[error("Failed to make request to beatport.com")]
    RequestError(#[from] reqwest::Error),

    #[error("Missing release ID for track")]
    MissingRelaseId,

    #[error("Problem during authentication: {0}")]
    AuthenticationError(String),
}

#[derive(Debug, PartialEq)]
pub struct BeatportTrackInfo {
    catalog_number: Option<String>,
    label: Option<String>,
    track_number: Option<u64>,
    track_total: Option<u64>,
    genre: Option<String>,
}

impl BeatportTrackInfo {
    /// Updates the provided [`TrackInfo`] with information from Beatport
    pub fn update_track_fields(&self, fields: &mut TrackFields) {
        if let Some(ref catalog_number) = self.catalog_number {
            fields.catalog_id = Some(catalog_number.clone());
        }

        if let Some(ref label) = self.label {
            fields.publisher = Some(label.clone());
        }

        if let Some(ref genre) = self.genre {
            fields.genre = Some(genre.clone());
        }

        // Update track number if we have both track_number and track_total
        if let (Some(track_number), Some(track_total)) = (self.track_number, self.track_total) {
            if track_number == 1 && track_total == 1 {
                // Single track release - clear track and disc fields
                fields.track = None;
                fields.disc = None;
            } else {
                fields.track = Some(CountField::Valid(Count {
                    number: track_number as u8,
                    total: track_total as u8,
                }));
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct Unauthenticated;

#[derive(Clone, Default)]
pub struct Authenticated {
    token: String,
}

impl Authenticated {
    fn new(token: String) -> Self {
        Self { token }
    }

    fn token(&self) -> &str {
        &self.token
    }
}

/// The username and password credentials required to authenticate with the beatport API to
/// authorize the beatport client. These are the same username and password used to login.
#[derive(Clone)]
pub struct BeatportCredentials {
    pub username: String,
    pub password: String,
}

pub struct BeatportSource<AuthState = Unauthenticated> {
    client: reqwest::Client,
    base_url_apis: String,
    auth_state: AuthState,
    genre_translation: Option<HashMap<String, String>>,
}

impl<AuthState> Default for BeatportSource<AuthState>
where
    AuthState: Default,
{
    fn default() -> Self {
        let client = reqwest::ClientBuilder::new()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        Self {
            client,
            base_url_apis: "https://api.beatport.com".to_string(),
            auth_state: AuthState::default(),
            genre_translation: None,
        }
    }
}

// OAuth constants extracted from Beatport API docs example JavaScript app
// since public API application registration is not available
static OAUTH_CLIENT_ID: &str = "0GIvkCltVIuPkkwSJHp6NDb3s0potTjLBQr388Dd";
static OAUTH_REDIRECT_PATH: &str = "/v4/auth/o/post-message/";

impl<AuthState> BeatportSource<AuthState> {
    /// Authenticate with Beatport using standard OAuth flow with username/password credentials.
    async fn get_token(
        &self,
        credentials: impl Borrow<BeatportCredentials>,
    ) -> Result<String, BeatportApiError> {
        let redirect_uri = &format!("{}{}", self.base_url_apis, OAUTH_REDIRECT_PATH);
        let creds = credentials.borrow();

        // Step 1: Login with username/password
        let login_response = self
            .client
            .post(format!("{}/v4/auth/login/", self.base_url_apis))
            .json(&json!({
                "username": creds.username,
                "password": creds.password
            }))
            .send()
            .await?;

        if !login_response.status().is_success() {
            return Err(BeatportApiError::AuthenticationError(
                "Login failed".to_string(),
            ));
        }

        // Step 2: Get authorization code
        let auth_response = self
            .client
            .get(format!("{}/v4/auth/o/authorize/", self.base_url_apis))
            .query(&[
                ("response_type", "code"),
                ("client_id", OAUTH_CLIENT_ID),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await?;

        // Extract authorization code from the redirect location
        let status = auth_response.status();
        let headers = auth_response.headers().clone();

        if !status.is_success() && !status.is_redirection() {
            return Err(BeatportApiError::AuthenticationError(
                "Authorization failed".to_string(),
            ));
        }

        let location_header = headers
            .get("location")
            .and_then(|h| h.to_str().ok())
            .ok_or(BeatportApiError::AuthenticationError(
                "Missing location header in authorization response".to_string(),
            ))?;

        let auth_code = url::Url::parse(&format!("{}{}", self.base_url_apis, location_header))
            .map_err(|_| {
                BeatportApiError::AuthenticationError("Failed to parse redirect URL".to_string())
            })?
            .query_pairs()
            .find(|(key, _)| key == "code")
            .map(|(_, value)| value.into_owned())
            .ok_or(BeatportApiError::AuthenticationError(
                "Authorization code not found in redirect URL".to_string(),
            ))?;

        // Step 3: Exchange authorization code for access token
        let token_resp = self
            .client
            .post(format!("{}/v4/auth/o/token/", self.base_url_apis))
            .form(&[
                ("code", auth_code.as_str()),
                ("grant_type", "authorization_code"),
                ("redirect_uri", redirect_uri),
                ("client_id", OAUTH_CLIENT_ID),
            ])
            .send()
            .await?;

        let token_response: Value = token_resp.json().await?;

        let access_token = token_response["access_token"]
            .as_str()
            .map(str::to_string)
            .ok_or(BeatportApiError::AuthenticationError(
                "Access token not found in token response".to_string(),
            ))?;

        Ok(access_token)
    }
}

impl BeatportSource<Unauthenticated> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Authenticate the BeatportSource client given Beatport.com credentials.
    pub async fn authenticate(
        &self,
        credentials: impl Borrow<BeatportCredentials>,
    ) -> Result<BeatportSource<Authenticated>, BeatportApiError> {
        let token = self.get_token(credentials).await?;
        Ok(BeatportSource {
            auth_state: Authenticated { token },
            ..Default::default()
        })
    }
}

impl BeatportSource<Authenticated> {
    /// Get the authentication token
    pub fn token(&self) -> &str {
        self.auth_state.token()
    }

    /// Update the authentication token in this client.
    pub async fn reauthenticate(
        mut self,
        credentials: impl Borrow<BeatportCredentials>,
    ) -> Result<Self, BeatportApiError> {
        self.auth_state.token = self.get_token(credentials).await?;
        Ok(self)
    }

    /// Retrieve track details from beatport.
    pub async fn fetch_track_info(
        &self,
        track_id: u32,
    ) -> Result<BeatportTrackInfo, BeatportApiError> {
        let token = &self.auth_state.token;

        let url = format!("{}/v4/catalog/tracks/{}/", self.base_url_apis, track_id);
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

        let url = format!("{}/v4/catalog/releases/{}/", self.base_url_apis, release_id);
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

    use super::{
        Authenticated, BeatportCredentials, BeatportSource, BeatportTrackInfo, OAUTH_CLIENT_ID,
        try_extract_track_id, try_extract_url,
    };
    use crate::tests::read_fixture;

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

    #[test]
    fn test_extract_url() {
        use id3::{Content, Frame, Tag, TagLike};

        let beatport_url = "https://www.beatport.com/track/move-feat-malachiii/19119572";

        // Create a tag with proper WOAF frame containing a Beatport URL
        {
            let mut tag = Tag::new();
            tag.add_frame(Frame::with_content(
                "WOAF",
                Content::Link(beatport_url.to_string()),
            ));
            assert_eq!(try_extract_url(&tag), Some(beatport_url.to_string()));
        };

        // Non-Beatport URL should return None
        {
            let mut tag = Tag::new();
            tag.add_frame(Frame::with_content(
                "WOAF",
                Content::Link("https://www.spotify.com/track/12345".to_string()),
            ));
            assert_eq!(try_extract_url(&tag), None);
        };

        // Empty tag should return None
        assert_eq!(try_extract_url(&Tag::new()), None);
    }

    #[tokio::test]
    async fn test_authenticate() {
        let server = MockServer::start();

        // Mock login endpoint
        server.mock(|when, then| {
            when.method(POST)
                .path("/v4/auth/login/")
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "username": "evan",
                    "password": "hunter2"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .body(read_fixture("beatport_login.json"));
        });

        // Mock authorization endpoint
        server.mock(|when, then| {
            when.method(GET)
                .path("/v4/auth/o/authorize/")
                .query_param("response_type", "code")
                .query_param("client_id", OAUTH_CLIENT_ID);
            then.status(302).header(
                "location",
                "/v4/auth/o/post-message/?code=test-auth-code-123&target=https://api.beatport.com",
            );
        });

        // Mock token exchange endpoint
        server.mock(|when, then| {
            when.method(POST)
                .path("/v4/auth/o/token/")
                .x_www_form_urlencoded_tuple("code", "test-auth-code-123")
                .x_www_form_urlencoded_tuple("grant_type", "authorization_code")
                .x_www_form_urlencoded_tuple("client_id", OAUTH_CLIENT_ID);
            then.status(200)
                .header("content-type", "application/json")
                .body(read_fixture("beatport_token.json"));
        });

        let beatport = BeatportSource {
            base_url_apis: server.base_url(),
            ..Default::default()
        }
        .authenticate(BeatportCredentials {
            username: "evan".to_string(),
            password: "hunter2".to_string(),
        })
        .await
        .unwrap();

        assert_eq!(beatport.auth_state.token, "test-access-token-123");
    }

    #[test]
    fn test_update_track_fields() {
        use crate::{
            fields::{Count, CountField},
            track::TrackFields,
        };

        let mut fields = TrackFields::default();

        // Test with multi-track release
        let info = BeatportTrackInfo {
            catalog_number: Some("TEST123".to_string()),
            label: Some("Test Label".to_string()),
            track_number: Some(2),
            track_total: Some(5),
            genre: Some("Test Genre".to_string()),
        };

        info.update_track_fields(&mut fields);

        assert_eq!(fields.catalog_id, Some("TEST123".to_string()));
        assert_eq!(fields.publisher, Some("Test Label".to_string()));
        assert_eq!(fields.genre, Some("Test Genre".to_string()));
        assert_eq!(
            fields.track,
            Some(CountField::Valid(Count {
                number: 2u8,
                total: 5u8
            }))
        );

        // Test with single track release (1/1)
        let single_info = BeatportTrackInfo {
            catalog_number: Some("SINGLE456".to_string()),
            label: Some("Single Label".to_string()),
            track_number: Some(1),
            track_total: Some(1),
            genre: Some("Single Genre".to_string()),
        };

        let mut single_fields = TrackFields::default();
        single_info.update_track_fields(&mut single_fields);

        assert_eq!(single_fields.catalog_id, Some("SINGLE456".to_string()));
        assert_eq!(single_fields.publisher, Some("Single Label".to_string()));
        assert_eq!(single_fields.genre, Some("Single Genre".to_string()));
        assert_eq!(single_fields.track, None);
        assert_eq!(single_fields.disc, None);
    }

    #[tokio::test]
    async fn test_fetch_track_info() {
        let server = MockServer::start();

        let token = "1d7AICC9GxVcJsi1VnKrYJGfVeRShD";

        server.mock(|when, then| {
            when.method(GET)
                .path("/v4/catalog/tracks/1234/")
                .header("Authorization", format!("Bearer {}", token));
            then.status(200)
                .header("content-type", "application/json")
                .body(read_fixture("beatport_track.json"));
        });
        server.mock(|when, then| {
            when.method(GET)
                .path("/v4/catalog/releases/439354/")
                .header("Authorization", format!("Bearer {}", token));
            then.status(200)
                .header("content-type", "application/json")
                .body(read_fixture("beatport_release.json"));
        });

        let source = BeatportSource {
            auth_state: Authenticated::new(token.to_string()),
            base_url_apis: server.base_url(),
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
