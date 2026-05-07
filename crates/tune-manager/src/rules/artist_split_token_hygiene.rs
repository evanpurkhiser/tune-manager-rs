use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "artist.split-token-hygiene";
const DESCRIPTION: &str = indoc::indoc! {r#"
Artist connector syntax must be clean with no dangling or duplicated separators.

Valid:
- A & B
- A, B & C

Invalid:
- A ,  B (bad spacing)
- A & & B (duplicate separator)
"#};

static BAD_HYGIENE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\s,)|(,\s*,)|(&\s*&)|(\s{2,})|(^[,&\s]+)|([,&\s]+$)").unwrap());

pub struct ArtistSplitTokenHygieneRule;

impl TrackRule for ArtistSplitTokenHygieneRule {
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
        if BAD_HYGIENE_RE.is_match(artist) {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Artist connector hygiene is invalid",
            )];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::ArtistSplitTokenHygieneRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.tags.artist = Some("A & B".to_string());
        assert!(ArtistSplitTokenHygieneRule.check(&track).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.artist = Some("A ,  B".to_string());
        assert_eq!(ArtistSplitTokenHygieneRule.check(&track).len(), 1);
    }
}
