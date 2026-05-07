use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "artist.separator-standardization",
    description: r#"
        Artist collaboration separators must use canonical tokens.

        Valid:
        - A & B
        - A vs B

        Invalid:
        - A and B (use &)
        - A vs. B (use vs without period)
        - A versus B (use vs)
    "#,
};

static NON_CANON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s(and|vs\.|versus)\s").unwrap());

// TODO: apply this rule to the `remixer` field too. The cleanest path is a
// shared `ArtistField` selector enum and registering this rule twice.
pub struct ArtistSeparatorStandardizationRule;

impl TrackRule for ArtistSeparatorStandardizationRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(artist) = track.tags.artist.as_deref() else {
            return vec![];
        };
        if !NON_CANON_RE.is_match(artist) {
            return vec![];
        }
        vec![
            self.error("Artist connectors are not canonical")
                .with_fix(|track| {
                    if let Some(artist) = track.tags.artist.as_deref() {
                        track.tags.artist = Some(standardize_separators(artist));
                    }
                }),
        ]
    }
}

fn standardize_separators(artist: &str) -> String {
    NON_CANON_RE
        .replace_all(artist, |caps: &regex::Captures| {
            match caps[1].to_ascii_lowercase().as_str() {
                "and" => " & ".to_string(),
                "vs." | "versus" => " vs ".to_string(),
                _ => caps[0].to_string(),
            }
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::ArtistSeparatorStandardizationRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.tags.artist = Some("A & B".to_string());
        assert!(ArtistSeparatorStandardizationRule.check(&track).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.artist = Some("A and B".to_string());
        assert_eq!(ArtistSeparatorStandardizationRule.check(&track).len(), 1);
    }

    #[test]
    fn fail_versus() {
        let mut track = make_track();
        track.tags.artist = Some("A versus B".to_string());
        assert_eq!(ArtistSeparatorStandardizationRule.check(&track).len(), 1);
    }

    fn fixed_artist(input: &str) -> String {
        let mut track = make_track();
        track.tags.artist = Some(input.to_string());
        let violations = ArtistSeparatorStandardizationRule.check(&track);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        track.tags.artist.unwrap()
    }

    #[test]
    fn fix_and_to_ampersand() {
        assert_eq!(fixed_artist("A and B"), "A & B");
    }

    #[test]
    fn fix_vs_dot_to_vs() {
        assert_eq!(fixed_artist("A vs. B"), "A vs B");
    }

    #[test]
    fn fix_versus_to_vs() {
        assert_eq!(fixed_artist("A versus B"), "A vs B");
    }

    #[test]
    fn fix_mixed_in_one_pass() {
        assert_eq!(fixed_artist("A and B versus C"), "A & B vs C");
    }
}
