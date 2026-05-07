use std::{collections::HashSet, sync::LazyLock};

use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ResponseFormatJsonSchema,
        responses::{
            Content, CreateResponseArgs, Input, InputContent, InputItem, InputMessage,
            OutputContent, Role, TextConfig, TextResponseFormat,
        },
    },
};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::{track::Track, track::TrackTags};

mod schema_types {
    typify::import_types!("src/services/schema/track-prompt.json");
}

pub type TrackResponse = schema_types::Track;

impl schema_types::Track {
    /// Updates the provided TrackTags with information from AI processing
    pub fn update_track_tags(&self, tags: &mut TrackTags) {
        tags.artist = Some(self.artist.clone().into());
        tags.title = Some(self.title.clone().into());
        tags.album = self.album.clone().into();
        tags.remixer = self.remixer.clone().into();
        tags.publisher = self.publisher.clone().into();
        tags.catalog_id = self.catalog_id.clone().into();
        tags.genre = self.genre.clone().into();
        tags.year = self.year.map(|y| y.to_string());

        // Parse disc and track fields from AI response
        tags.disc = self.disc.as_ref().and_then(|d| d.parse().ok());
        tags.track = self.track.as_ref().and_then(|d| d.parse().ok());

        // Note: key and bpm are preserved from previous stages and not updated by AI
    }
}

static FORMAT: LazyLock<TextResponseFormat> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(include_str!("schema/track-prompt.json")).unwrap();

    let format = ResponseFormatJsonSchema {
        name: "PromptResponse".to_string(),
        description: None,
        schema: Some(schema),
        strict: Some(true),
    };
    TextResponseFormat::JsonSchema(format)
});

const SYSTEM_PROMPT: &str = r#"
You are a DJ who meticulously and expertly organizes track metadata.

You think like a record collector who values accuracy, consistency, and clean formatting. Your goal
is to produce perfectly normalized metadata for electronic music tracks — thoughtful, consistent,
and free of redundancy or guesswork. If information is missing or unclear, use null rather than
inventing data.

You MUST produce JSON that matches the provided schema.

ARTIST FORMATTING
- The vocalist belongs only in the artist field, never in the title.
- Append vocalists as `Ft. Name` (with a period).
- Join collaborating artists with `,`, `vs`, or `&`. Preserve the writer's
  choice when it appears intentional; mixed separators are allowed (e.g.
  `Aly & Fila, Lostly` or `Technikore vs Dougal & Gammer`).
- Apply the same formatting rules to remixers.

TITLE CLEANUP
- Keep descriptors like `(Extended Mix)` and `(Artist Name Remix)`.
- Remove `(Original Mix)` or similar placeholders.
- Do not duplicate artist or remixer info inside the title.

REMIXER HANDLING
- If the title includes `(Artist Name Remix)`, set `remixer` to that artist string using proper formatting.
- Otherwise, `remixer` is null.

TRACK & DISC FIELDS
- `track`: use `t/a` where `t` is track number and `a` is total tracks in that disc/release. For singles, leave as null (never `1/1`).
- `disc`: use `d/a` only when applicable. For single-disc multi-track releases, use `1/1`; for true singles, use null.
- If one file clearly belongs to a multi-track release, treat it as part of that release (keep album, disc, track).

ALBUM / PUBLISHER / CATALOG / YEAR / GENRE
- `album` MUST be null only for singles (track = 1/1).
- `publisher`, `catalog_id`, and `year` may be null if unknown.
- Use a single free-text genre. Prefer “Hardcore”, “Trance”, or “Hard Trance” when relevant, but choose what best fits.
- Tracks from the same release share the same `publisher` and `catalog_id` when available.

CONSISTENCY
- Choose the most standard professional DJ metadata convention when uncertain.
- Never invent or assume — only include information clearly indicated or industry-standard.
"#;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("CSV serialization failed")]
    Csv(#[from] csv::Error),

    #[error("String encoding error")]
    Encoding(#[from] std::string::FromUtf8Error),

    #[error("OpenAI API request failed")]
    OpenAi(#[from] async_openai::error::OpenAIError),

    #[error("JSON parsing failed")]
    Json(#[from] serde_json::Error),

    #[error("No text content found in response")]
    NoTextContent,

    #[error("Input track missing media_hash: {file_path}")]
    MissingMediaHash { file_path: String },

    #[error("AI response missing media_hashes: {missing:?}")]
    ResponseMissingMediaHashes { missing: Vec<String> },
}

#[derive(Serialize)]
struct TrackCsv {
    media_hash: String,
    file_path: String,
    artist: Option<String>,
    title: Option<String>,
    album: Option<String>,
    remixer: Option<String>,
    publisher: Option<String>,
    catalog_id: Option<String>,
    year: Option<String>,
    genre: Option<String>,
    disc: Option<String>,
    track: Option<String>,
}

impl TryFrom<&Track> for TrackCsv {
    type Error = AiError;

    fn try_from(track: &Track) -> Result<Self, Self::Error> {
        let tags = &track.tags;
        let file_path = track.metadata.file_path.to_string_lossy().to_string();

        let media_hash = tags.media_hash.clone().ok_or(AiError::MissingMediaHash {
            file_path: file_path.clone(),
        })?;

        Ok(Self {
            media_hash,
            file_path,
            artist: tags.artist.clone(),
            title: tags.title.clone(),
            album: tags.album.clone(),
            remixer: tags.remixer.clone(),
            publisher: tags.publisher.clone(),
            catalog_id: tags.catalog_id.clone(),
            year: tags.year.clone(),
            genre: tags.genre.clone(),
            disc: tags.disc.as_ref().map(|d| d.to_string()),
            track: tags.track.as_ref().map(|t| t.to_string()),
        })
    }
}

fn tracks_to_csv(tracks: &[Track]) -> Result<String, AiError> {
    let mut csv_writer = csv::Writer::from_writer(Vec::new());

    for track in tracks {
        csv_writer.serialize(TrackCsv::try_from(track)?)?;
    }

    let csv_data = String::from_utf8(csv_writer.into_inner().unwrap())?;
    Ok(csv_data)
}

fn validate_response_media_hashes(
    input_tracks: &[Track],
    response: &schema_types::PromptResponse,
) -> Result<(), AiError> {
    let input_hashes: HashSet<_> = input_tracks
        .iter()
        .filter_map(|t| t.tags.media_hash.as_ref())
        .collect();

    let response_hashes: HashSet<_> = response.tracks.iter().map(|t| &t.media_hash).collect();

    let missing: Vec<String> = input_hashes
        .difference(&response_hashes)
        .map(|s| s.to_string())
        .collect();

    if !missing.is_empty() {
        return Err(AiError::ResponseMissingMediaHashes { missing });
    }

    Ok(())
}

pub async fn process_tracks(
    client: &Client<OpenAIConfig>,
    tracks: Vec<Track>,
) -> Result<schema_types::PromptResponse, AiError> {
    let csv_data = tracks_to_csv(&tracks)?;

    let request = CreateResponseArgs::default()
        .text(TextConfig {
            format: FORMAT.clone(),
            verbosity: None,
        })
        .model("o4-mini")
        .input(Input::Items(vec![
            InputItem::Message(InputMessage {
                kind: Default::default(),
                role: Role::System,
                content: InputContent::TextInput(SYSTEM_PROMPT.to_string()),
            }),
            InputItem::Message(InputMessage {
                kind: Default::default(),
                role: Role::User,
                content: InputContent::TextInput(csv_data),
            }),
        ]))
        .build()?;

    let resp = client.responses().create(request).await?;

    // Extract the text from the response
    let text_content = resp
        .output
        .iter()
        .filter_map(|item| match item {
            OutputContent::Message(msg) => Some(msg),
            _ => None,
        })
        .flat_map(|msg| &msg.content)
        .filter_map(|content| match content {
            Content::OutputText(text) => Some(&text.text),
            _ => None,
        })
        .next()
        .ok_or(AiError::NoTextContent)?;

    let payload: schema_types::PromptResponse = serde_json::from_str(text_content)?;

    validate_response_media_hashes(&tracks, &payload)?;

    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fields::CountField,
        track::{TrackMetadaata, TrackTags},
    };
    use std::path::PathBuf;

    fn create_test_track() -> Track {
        Track {
            metadata: TrackMetadaata {
                file_path: PathBuf::from("/test/path/track.mp3"),
                mtime: 1234567890,
            },
            tags: TrackTags {
                media_hash: Some("098f6bcd4621d373cade4e832627b4f6".to_string()),
                artist: Some("Test Artist".to_string()),
                title: Some("Test Title".to_string()),
                album: Some("Test Album".to_string()),
                remixer: Some("Test Remixer".to_string()),
                publisher: Some("Test Label".to_string()),
                catalog_id: Some("TEST001".to_string()),
                year: Some("2023".to_string()),
                genre: Some("Electronic".to_string()),
                key: Some("Am".to_string()),
                bpm: Some("128".to_string()),
                disc: Some(CountField::Valid(crate::fields::Count {
                    number: 1,
                    total: 2,
                })),
                track: Some(CountField::Valid(crate::fields::Count {
                    number: 3,
                    total: 10,
                })),
            },
        }
    }

    #[test]
    fn test_tracks_to_csv_single_track() {
        let track = create_test_track();
        let result = tracks_to_csv(&[track]).unwrap();

        // Should contain CSV headers and one data row
        let lines: Vec<&str> = result.trim().split('\n').collect();
        assert_eq!(lines.len(), 2, "Should have header + 1 data row");

        // Check that it contains expected data
        assert!(result.contains("Test Artist"));
        assert!(result.contains("Test Title"));
        assert!(result.contains("Test Album"));
        assert!(result.contains("TEST001"));
    }

    #[test]
    fn test_update_track_tags() {
        use crate::{
            fields::{Count, CountField},
            track::TrackTags,
        };

        let mut tags = TrackTags::default();

        // Create AI track response
        let ai_track = schema_types::Track {
            media_hash: "4092e62ffa902b289811c30f3d8d3794".to_string(),
            artist: "Test Artist Ft. Vocalist".to_string().into(),
            title: "Test Title (Extended Mix)".to_string().into(),
            album: Some("Test Album".to_string()).into(),
            remixer: Some("Test Remixer".to_string()).into(),
            publisher: Some("Test Label".to_string()).into(),
            catalog_id: Some("TEST123".to_string()).into(),
            year: Some(2023).into(),
            genre: Some("Hardcore".to_string()).into(),
            disc: Some("1/2".to_string()).into(),
            track: Some("3/10".to_string()).into(),
        };

        // Set some initial values that should be preserved
        tags.key = Some("10A".to_string());
        tags.bpm = Some("140".to_string());

        ai_track.update_track_tags(&mut tags);

        // Check AI updates
        assert_eq!(tags.artist, Some("Test Artist Ft. Vocalist".to_string()));
        assert_eq!(tags.title, Some("Test Title (Extended Mix)".to_string()));
        assert_eq!(tags.album, Some("Test Album".to_string()));
        assert_eq!(tags.remixer, Some("Test Remixer".to_string()));
        assert_eq!(tags.publisher, Some("Test Label".to_string()));
        assert_eq!(tags.catalog_id, Some("TEST123".to_string()));
        assert_eq!(tags.year, Some("2023".to_string()));
        assert_eq!(tags.genre, Some("Hardcore".to_string()));
        assert_eq!(
            tags.disc,
            Some(CountField::Valid(Count {
                number: 1u8,
                total: 2u8
            }))
        );
        assert_eq!(
            tags.track,
            Some(CountField::Valid(Count {
                number: 3u8,
                total: 10u8
            }))
        );

        // Check preserved values
        assert_eq!(tags.key, Some("10A".to_string()));
        assert_eq!(tags.bpm, Some("140".to_string()));
    }

    #[test]
    fn test_update_track_tags_single_track() {
        use crate::track::TrackTags;

        let mut tags = TrackTags::default();

        // Create AI track response for a single
        let ai_track = schema_types::Track {
            media_hash: "4092e62ffa902b289811c30f3d8d3794".to_string(),
            artist: "Single Artist".to_string().into(),
            title: "Single Title".to_string().into(),
            album: None.into(),
            remixer: None.into(),
            publisher: Some("Single Label".to_string()).into(),
            catalog_id: Some("SINGLE001".to_string()).into(),
            year: Some(2024).into(),
            genre: Some("Trance".to_string()).into(),
            disc: None.into(),
            track: None.into(),
        };

        ai_track.update_track_tags(&mut tags);

        assert_eq!(tags.artist, Some("Single Artist".to_string()));
        assert_eq!(tags.title, Some("Single Title".to_string()));
        assert_eq!(tags.album, None);
        assert_eq!(tags.remixer, None);
        assert_eq!(tags.publisher, Some("Single Label".to_string()));
        assert_eq!(tags.catalog_id, Some("SINGLE001".to_string()));
        assert_eq!(tags.year, Some("2024".to_string()));
        assert_eq!(tags.genre, Some("Trance".to_string()));
        assert_eq!(tags.disc, None);
        assert_eq!(tags.track, None);
    }

    #[test]
    fn test_tracks_to_csv_multiple_tracks() {
        let track1 = create_test_track();
        let mut track2 = create_test_track();
        track2.tags.artist = Some("Artist Two".to_string());
        track2.tags.title = Some("Title Two".to_string());

        let result = tracks_to_csv(&[track1, track2]).unwrap();

        let lines: Vec<&str> = result.trim().split('\n').collect();
        assert_eq!(lines.len(), 3, "Should have header + 2 data rows");

        assert!(result.contains("Test Artist"));
        assert!(result.contains("Artist Two"));
        assert!(result.contains("Title Two"));
    }
}
