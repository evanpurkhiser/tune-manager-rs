use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rule_metadata,
    linter::{RuleMetadata, RuleViolation, TrackRule},
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
    autofix_notes: r#"
        Replaces non-canonical featuring tokens (`feat.`, `featuring`,
        `ft.`, `ft`, case-insensitive) with `Ft.`.
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
        let Some(cap) = FEAT_VARIANT_RE.captures(artist) else {
            return vec![];
        };
        let token = cap
            .get(1)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        if token == "ft." {
            return vec![];
        }
        vec![
            self.error("Featuring token should be canonical Ft.")
                .with_fix(|track| {
                    if let Some(artist) = track.tags.artist.as_deref() {
                        track.tags.artist = Some(standardize_feat(artist));
                    }
                }),
        ]
    }
}

fn standardize_feat(artist: &str) -> String {
    FEAT_VARIANT_RE.replace_all(artist, " Ft. ").to_string()
}

#[cfg(test)]
mod tests {
    use super::ArtistFeatStandardizationRule;
    use crate::linter::{TrackRule, test_utils::make_track};

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

    fn fixed_artist(input: &str) -> String {
        let mut track = make_track();
        track.tags.artist = Some(input.to_string());
        let violations = ArtistFeatStandardizationRule.check(&track);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        track.tags.artist.unwrap()
    }

    #[test]
    fn fix_feat_dot() {
        assert_eq!(fixed_artist("A feat. B"), "A Ft. B");
    }

    #[test]
    fn fix_featuring() {
        assert_eq!(fixed_artist("A featuring B"), "A Ft. B");
    }

    #[test]
    fn fix_ft_no_dot() {
        assert_eq!(fixed_artist("A ft B"), "A Ft. B");
    }

    #[test]
    fn fix_uppercase() {
        assert_eq!(fixed_artist("A FEAT B"), "A Ft. B");
    }
}
