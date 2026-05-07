use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "artist.feat-standardization",
    description: r#"
        Featuring token in artist field must be canonical Ft.

        Valid:
        - Artist A Ft. Artist B
        - Artist A

        Invalid:
        - Artist A feat. Artist B (use Ft.)
        - Artist A featuring Artist B (use Ft.)
    "#,
};

static FEAT_VARIANT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s(featuring|feat\.?|ft\.?)\s").unwrap());

pub struct ArtistFeatStandardizationRule;

impl TrackRule for ArtistFeatStandardizationRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(artist) = track.tags.artist.as_deref() else {
            return vec![];
        };
        if let Some(cap) = FEAT_VARIANT_RE.captures(artist) {
            let token = cap
                .get(1)
                .map(|m| m.as_str().to_ascii_lowercase())
                .unwrap_or_default();
            if token != "ft." {
                return vec![self.error("Featuring token should be canonical Ft.")];
            }
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::ArtistFeatStandardizationRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.tags.artist = Some("A Ft. B".to_string());
        assert!(ArtistFeatStandardizationRule.check(&track).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.artist = Some("A feat. B".to_string());
        assert_eq!(ArtistFeatStandardizationRule.check(&track).len(), 1);
    }
}
