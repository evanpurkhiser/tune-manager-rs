use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "title.no-original-mix";
const DESCRIPTION: &str = indoc::indoc! {r#"
Title must not include an Original Mix marker.

Valid:
- Song
- Song (Artist Remix)

Invalid:
- Song (Original Mix) (original mix label should be removed)
- Song (ORIGINAL MIX) (case-insensitive match)
"#};

static ORIGINAL_MIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(original mix\)").unwrap());

pub struct TitleNoOriginalMixRule;

impl TrackRule for TitleNoOriginalMixRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(title) = track.tags.title.as_deref() else {
            return vec![];
        };
        if ORIGINAL_MIX_RE.is_match(&title.to_ascii_lowercase()) {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Title should not include (Original Mix)",
            )];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::TitleNoOriginalMixRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(TitleNoOriginalMixRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.title = Some("Song (Original Mix)".to_string());
        assert_eq!(TitleNoOriginalMixRule.check(&track).len(), 1);
    }
}
