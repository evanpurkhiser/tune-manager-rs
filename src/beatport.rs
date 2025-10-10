use std::{collections::HashMap, sync::LazyLock};

use regex::Regex;
use serde_json::Value;
use thiserror::Error;

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

#[derive(Error, Debug)]
pub enum BeatportApiError {
    #[error("Failed to make request to beatport.com")]
    RequestError(#[from] reqwest::Error),

    #[error("Missing release ID for track")]
    MissingRelaseId,

    #[error("Problem during authentication")]
    AuthenticationError,
}

#[derive(Debug, PartialEq)]
pub struct BeatportTrackInfo {
    catalog_number: Option<String>,
    label: Option<String>,
    track_number: Option<u64>,
    track_total: Option<u64>,
    genre: Option<String>,
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
}

/// The username and password credentials required to authenticate with the beatport API to
/// authorize the beatport client. These are the same username and password used to login.
pub struct BeatportCredentials {
    pub username: String,
    pub password: String,
}

pub struct BeatportSource<AuthState = Unauthenticated> {
    client: reqwest::Client,
    base_url_auth: String,
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
            .build()
            .unwrap();

        Self {
            client,
            base_url_auth: "https://www.beatport.com".to_string(),
            base_url_apis: "https://api.beatport.com".to_string(),
            auth_state: AuthState::default(),
            genre_translation: None,
        }
    }
}

impl<AuthState> BeatportSource<AuthState> {
    /// Authneticate with the production beatport.com app using username and password credentials.
    async fn get_token(
        &self,
        credentials: BeatportCredentials,
    ) -> Result<String, BeatportApiError> {
        // XXX: This authentication does NOT use the typical oAuth flow that you might expect,
        // given that Beatport does document their APIs. There's no easy way to obtain an oAuth
        // token, so instead we use the production www.beatport.com APIs to do authentication,
        // which gives us a token that can be used with api.beatport.com
        //
        // This is wh there is a `base_url_auth` and a `base_url_apis`.

        // Retrieve the CSRF token used when authenticating
        let csrf_resp: Value = self
            .client
            .get(format!("{}/api/auth/csrf", self.base_url_auth))
            .send()
            .await?
            .json()
            .await?;
        let csrf_token = csrf_resp["csrfToken"]
            .as_str()
            .map(str::to_string)
            .ok_or(BeatportApiError::AuthenticationError)?;

        // Do authentication using credentials. This will resply with a set-cookie that reqwest
        // will use in the next call to retrieve the auth token.
        self.client
            .post(format!("{}/api/auth/callback/beatport", self.base_url_auth))
            .form(&[
                ("username", credentials.username),
                ("password", credentials.password),
                ("csrfToken", csrf_token),
                ("json", "true".to_string()),
                ("redirect", "false".to_string()),
            ])
            .send()
            .await?;

        // Request session details to get our auth token.
        let session_resp: Value = self
            .client
            .get(format!("{}/api/auth/session", self.base_url_auth))
            .send()
            .await?
            .json()
            .await?;
        let token = session_resp["token"]["accessToken"]
            .as_str()
            .map(str::to_string)
            .ok_or(BeatportApiError::AuthenticationError)?;

        Ok(token)
    }
}

impl BeatportSource<Unauthenticated> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Authenticate the BeatportSource client given Beatport.com credentials.
    pub async fn authenticate(
        &self,
        credentials: BeatportCredentials,
    ) -> Result<BeatportSource<Authenticated>, BeatportApiError> {
        let token = self.get_token(credentials).await?;
        Ok(BeatportSource {
            auth_state: Authenticated { token },
            ..Default::default()
        })
    }
}

impl BeatportSource<Authenticated> {
    /// Update the authentication token in this client.
    pub async fn reauthenticate(
        mut self,
        credentials: BeatportCredentials,
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

        let url = format!("{}/v4/catalog/tracks/{}", self.base_url_apis, track_id);
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

        let url = format!("{}/v4/catalog/releases/{}", self.base_url_apis, release_id);
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
        Authenticated, BeatportCredentials, BeatportSource, BeatportTrackInfo, try_extract_track_id,
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

    #[tokio::test]
    async fn test_authenticate() {
        let server = MockServer::start();

        server.mock(|when, then| {
            when.method(GET).path("/api/auth/csrf");
            then.status(200)
                .header("content-type", "application/json")
                .body(read_fixture("beatport_csrf.json"));
        });
        server.mock(|when, then| {
            when.method(POST)
                .path("/api/auth/callback/beatport")
                .x_www_form_urlencoded_tuple("username", "evan")
                .x_www_form_urlencoded_tuple("password", "hunter2")
                .x_www_form_urlencoded_tuple("csrfToken", "example-csrf");
            then.status(200);
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/auth/session");
            then.status(200).body(read_fixture("beatport_session.json"));
        });

        let beatport = BeatportSource {
            base_url_auth: server.base_url(),
            base_url_apis: server.base_url(),
            ..Default::default()
        }
        .authenticate(BeatportCredentials {
            username: "evan".to_string(),
            password: "hunter2".to_string(),
        })
        .await
        .unwrap();

        assert_eq!(beatport.auth_state.token, "example-token");
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
            auth_state: Authenticated::new(token.to_string()),
            base_url_auth: server.base_url(),
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
