use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "bpm.numeric";
const DESCRIPTION: &str = indoc::indoc! {r#"
BPM must be numeric with strict decimal formatting.

Valid:
- 170
- 128.5
- 128.25

Invalid:
- fast (not numeric)
- 12x (contains non-numeric characters)
- 128.345 (more than two decimal places)
- 170.50 (trailing zero in decimal form)
- 170.00 (whole numbers should not be decimal)
"#};

static BPM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9]+(?:\.[0-9]?[1-9])?$").unwrap());

pub struct BpmNumericRule;

impl TrackRule for BpmNumericRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(bpm) = track.tags.bpm.as_deref() else {
            return vec![];
        };
        if BPM_RE.is_match(bpm) {
            return vec![];
        }
        vec![violation(
            RULE_ID,
            RuleSeverity::Warn,
            "BPM must be integer or decimal with 1-2 places and no trailing decimal zero",
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::BpmNumericRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(BpmNumericRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.bpm = Some("fast".to_string());
        assert_eq!(BpmNumericRule.check(&track).len(), 1);
    }

    #[test]
    fn fail_case_with_too_many_decimals() {
        let mut track = make_track();
        track.tags.bpm = Some("128.345".to_string());
        assert_eq!(BpmNumericRule.check(&track).len(), 1);
    }

    #[test]
    fn fail_case_with_whitespace() {
        let mut track = make_track();
        track.tags.bpm = Some(" 128.5 ".to_string());
        assert_eq!(BpmNumericRule.check(&track).len(), 1);
    }

    #[test]
    fn fail_case_with_trailing_zero_decimal() {
        let mut track = make_track();
        track.tags.bpm = Some("170.50".to_string());
        assert_eq!(BpmNumericRule.check(&track).len(), 1);
    }

    #[test]
    fn fail_case_with_double_zero_decimal() {
        let mut track = make_track();
        track.tags.bpm = Some("170.00".to_string());
        assert_eq!(BpmNumericRule.check(&track).len(), 1);
    }
}
