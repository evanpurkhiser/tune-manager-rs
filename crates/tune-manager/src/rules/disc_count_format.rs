use crate::{
    fields::CountField,
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "disc.count-format";
const DESCRIPTION: &str = indoc::indoc! {r#"
Disc count must be parseable and internally valid when present.

Valid:
- 01/02
- 1/1

Invalid:
- x/y (not parseable)
- 3/2 (disc number greater than total)
"#};

pub struct DiscCountFormatRule;

impl TrackRule for DiscCountFormatRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        match track.tags.disc.as_ref() {
            None => vec![],
            Some(CountField::Invalid(_)) => vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Disc number format is invalid",
            )],
            Some(CountField::Valid(c)) if c.number == 0 || c.total == 0 || c.number > c.total => {
                vec![violation(
                    RULE_ID,
                    RuleSeverity::Warn,
                    "Disc number is out of valid range",
                )]
            }
            Some(CountField::Valid(_)) => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DiscCountFormatRule;
    use crate::{
        fields::CountField,
        rules::{TrackRule, test_utils::make_track},
    };

    #[test]
    fn ok_case() {
        assert!(DiscCountFormatRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.disc = Some(CountField::Invalid("x".to_string()));
        assert_eq!(DiscCountFormatRule.check(&track).len(), 1);
    }
}
