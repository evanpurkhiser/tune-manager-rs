use std::sync::LazyLock;

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
    typify::import_types!("src/schema/track-prompt.json");
}

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
- Two artists: join with `&` or `vs` (preserve whichever form is present).
- Three or more artists: join with commas.
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
}

#[derive(Serialize)]
struct TrackCsv {
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

impl From<&Track> for TrackCsv {
    fn from(track: &Track) -> Self {
        Self {
            file_path: track.metadata.file_path.to_string_lossy().to_string(),
            artist: track.tags.artist.clone(),
            title: track.tags.title.clone(),
            album: track.tags.album.clone(),
            remixer: track.tags.remixer.clone(),
            publisher: track.tags.publisher.clone(),
            catalog_id: track.tags.catalog_id.clone(),
            year: track.tags.year.clone(),
            genre: track.tags.genre.clone(),
            disc: track.tags.disc.as_ref().map(|d| d.to_string()),
            track: track.tags.track.as_ref().map(|t| t.to_string()),
        }
    }
}

fn tracks_to_csv(tracks: &[Track]) -> Result<String, AiError> {
    let mut csv_writer = csv::Writer::from_writer(Vec::new());

    for track in tracks {
        csv_writer.serialize(TrackCsv::from(track))?;
    }

    let csv_data = String::from_utf8(csv_writer.into_inner().unwrap())?;
    Ok(csv_data)
}

pub async fn process_tracks(
    client: Client<OpenAIConfig>,
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
