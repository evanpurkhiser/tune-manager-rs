use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "title.no-featuring-token";
const DESCRIPTION: &str = indoc::indoc! {r#"
Title must not include featuring tokens; featuring belongs in artist field.

Valid:
- Song
- Song (Artist Remix)

Invalid:
- Song feat. Singer (featuring token in title)
- Song ft Singer (featuring token in title)
"#};

static FEAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(feat\.?|featuring|ft\.?)\b").unwrap());

pub struct TitleNoFeaturingTokenRule;

impl TrackRule for TitleNoFeaturingTokenRule {
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
        if FEAT_RE.is_match(title) {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Title should not include featuring token",
            )];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::TitleNoFeaturingTokenRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(TitleNoFeaturingTokenRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.title = Some("Song feat. Singer".to_string());
        assert_eq!(TitleNoFeaturingTokenRule.check(&track).len(), 1);
    }

    #[test]
    fn fail_case_ft() {
        let mut track = make_track();
        track.tags.title = Some("Song ft Singer".to_string());
        assert_eq!(TitleNoFeaturingTokenRule.check(&track).len(), 1);
    }
}
