use crate::track::Track;

pub mod album_requires_disc;
pub mod artist_feat_standardization;
pub mod artist_separator_standardization;
pub mod artist_split_token_hygiene;
pub mod bpm_numeric;
pub mod disc_count_format;
pub mod disc_requires_track;
pub mod file_supported_extension;
pub mod key_canonical_camelot;
pub mod meta_no_smart_quotes;
pub mod meta_publisher_catalog_pairing;
pub mod meta_remixer_title_consistency;
pub mod path_matches_canonical;
pub mod title_mix_suffix_style;
pub mod title_no_featuring_token;
pub mod title_no_original_mix;
pub mod track_count_format;
pub mod track_requires_disc;
pub mod year_format;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSeverity {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleViolation {
    pub rule_id: &'static str,
    pub severity: RuleSeverity,
    pub message: String,
}

pub trait TrackRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn description(&self) -> &'static str;

    fn check(&self, track: &Track) -> Vec<RuleViolation>;
}

pub fn track_only_rules() -> Vec<Box<dyn TrackRule>> {
    vec![
        Box::new(file_supported_extension::FileSupportedExtensionRule),
        Box::new(path_matches_canonical::PathMatchesCanonicalRule),
        Box::new(key_canonical_camelot::KeyCanonicalCamelotRule),
        Box::new(track_count_format::TrackCountFormatRule),
        Box::new(disc_count_format::DiscCountFormatRule),
        Box::new(album_requires_disc::AlbumRequiresDiscRule),
        Box::new(disc_requires_track::DiscRequiresTrackRule),
        Box::new(track_requires_disc::TrackRequiresDiscRule),
        Box::new(meta_publisher_catalog_pairing::MetaPublisherCatalogPairingRule),
        Box::new(meta_no_smart_quotes::MetaNoSmartQuotesRule),
        Box::new(title_mix_suffix_style::TitleMixSuffixStyleRule),
        Box::new(title_no_original_mix::TitleNoOriginalMixRule),
        Box::new(title_no_featuring_token::TitleNoFeaturingTokenRule),
        Box::new(meta_remixer_title_consistency::MetaRemixerTitleConsistencyRule),
        Box::new(artist_separator_standardization::ArtistSeparatorStandardizationRule),
        Box::new(artist_feat_standardization::ArtistFeatStandardizationRule),
        Box::new(artist_split_token_hygiene::ArtistSplitTokenHygieneRule),
        Box::new(year_format::YearFormatRule),
        Box::new(bpm_numeric::BpmNumericRule),
    ]
}

pub fn violation(
    rule_id: &'static str,
    severity: RuleSeverity,
    message: impl Into<String>,
) -> RuleViolation {
    RuleViolation {
        rule_id,
        severity,
        message: message.into(),
    }
}

#[cfg(test)]
pub mod test_utils {
    use std::path::PathBuf;

    use crate::{
        fields::CountField,
        track::{Track, TrackMetadaata, TrackTags},
    };

    pub fn make_track() -> Track {
        Track {
            metadata: TrackMetadaata {
                file_path: PathBuf::from(
                    "Publisher/[RLS] Album/Disc 2/01. [10A] Artist - Title.mp3",
                ),
                mtime: 1,
            },
            tags: TrackTags {
                media_hash: Some("abc123".to_string()),
                artist: Some("Artist".to_string()),
                title: Some("Title".to_string()),
                album: Some("Album".to_string()),
                remixer: Some("Remixer".to_string()),
                publisher: Some("Publisher".to_string()),
                catalog_id: Some("RLS".to_string()),
                year: Some("2015".to_string()),
                genre: Some("Genre".to_string()),
                key: Some("10A".to_string()),
                bpm: Some("170".to_string()),
                disc: Some("2/4".parse::<CountField>().unwrap()),
                track: Some("1/10".parse::<CountField>().unwrap()),
            },
        }
    }
}
