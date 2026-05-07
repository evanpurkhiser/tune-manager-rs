use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "disc.requires-track";
const DESCRIPTION: &str = indoc::indoc! {r#"
If disc is present, track must also be present.

Valid:
- disc=1/2, track=01/10
- disc missing, track missing

Invalid:
- disc=1/2, track missing (disc context without track index)
"#};

pub struct DiscRequiresTrackRule;

impl TrackRule for DiscRequiresTrackRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        if track.tags.disc.is_some() && track.tags.track.is_none() {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Disc is present but track is missing",
            )];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::DiscRequiresTrackRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(DiscRequiresTrackRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.track = None;
        assert_eq!(DiscRequiresTrackRule.check(&track).len(), 1);
    }
}
