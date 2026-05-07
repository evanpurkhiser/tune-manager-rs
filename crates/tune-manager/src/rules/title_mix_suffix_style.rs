use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "title.mix-suffix-style";
const DESCRIPTION: &str = indoc::indoc! {r#"
Mix/edit/version suffix in title must use canonical capitalization.

Valid:
- Song (Artist Remix)
- Song (Producer Edit)

Invalid:
- Song (artist remix) (non-canonical suffix style)
- Song (artist version) (non-canonical suffix style)
"#};

static MIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(([^\)]*?)\s(Remix|Edit|Mix|Version)\)").unwrap());
static FUZZY_MIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\(([^\)]*?)\s(remix|edit|mix|version)\)").unwrap());

pub struct TitleMixSuffixStyleRule;

impl TrackRule for TitleMixSuffixStyleRule {
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
        if MIX_RE.is_match(title) {
            return vec![];
        }

        if FUZZY_MIX_RE.is_match(title) {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Title mix suffix is not canonical",
            )];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::TitleMixSuffixStyleRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.tags.title = Some("Song (Artist Remix)".to_string());
        assert!(TitleMixSuffixStyleRule.check(&track).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.title = Some("Song (artist remix)".to_string());
        assert_eq!(TitleMixSuffixStyleRule.check(&track).len(), 1);
    }
}
