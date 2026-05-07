use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "artist.separator-standardization";
const DESCRIPTION: &str = indoc::indoc! {r#"
Artist collaboration separators must use canonical tokens.

Valid:
- A & B
- A vs B

Invalid:
- A and B (use &)
- A vs. B (use vs without period)
"#};

static NON_CANON_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s(and|vs\.)\s").unwrap());

pub struct ArtistSeparatorStandardizationRule;

impl TrackRule for ArtistSeparatorStandardizationRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(artist) = track.tags.artist.as_deref() else {
            return vec![];
        };
        if NON_CANON_RE.is_match(artist) {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Artist connectors are not canonical",
            )];
        }
        vec![]
    }
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
}
