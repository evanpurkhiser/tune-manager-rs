use crate::linter::TrackRule;

pub mod album_requires_disc;
pub mod artist_feat_standardization;
pub mod artist_separator_standardization;
pub mod artist_separator_structure;
pub mod bpm_numeric;
pub mod disc_count_format;
pub mod disc_requires_track;
pub mod file_supported_extension;
pub mod key_canonical_camelot;
pub mod meta_no_smart_quotes;
pub mod meta_publisher_catalog_pairing;
pub mod meta_remixer_title_consistency;
pub mod meta_required_fields_present;
pub mod path_matches_canonical;
pub mod title_mix_suffix_style;
pub mod title_no_featuring_token;
pub mod title_no_original_mix;
pub mod track_count_format;
pub mod track_requires_disc;
pub mod year_format;

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
        Box::new(meta_required_fields_present::MetaRequiredFieldsPresentRule),
        Box::new(meta_no_smart_quotes::MetaNoSmartQuotesRule),
        Box::new(title_mix_suffix_style::TitleMixSuffixStyleRule),
        Box::new(title_no_original_mix::TitleNoOriginalMixRule),
        Box::new(title_no_featuring_token::TitleNoFeaturingTokenRule),
        Box::new(meta_remixer_title_consistency::MetaRemixerTitleConsistencyRule),
        Box::new(artist_separator_standardization::ArtistSeparatorStandardizationRule),
        Box::new(artist_separator_structure::ArtistSeparatorStructureRule),
        Box::new(artist_feat_standardization::ArtistFeatStandardizationRule),
        Box::new(year_format::YearFormatRule),
        Box::new(bpm_numeric::BpmNumericRule),
    ]
}
