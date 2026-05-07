use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "track.requires-disc";
const DESCRIPTION: &str = indoc::indoc! {r#"
If track is present, disc must also be present.

Valid:
- track=01/10, disc=1/1
- track missing, disc missing

Invalid:
- track=01/10, disc missing (track count requires disc context)
"#};

pub struct TrackRequiresDiscRule;

impl TrackRule for TrackRequiresDiscRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        if track.tags.track.is_some() && track.tags.disc.is_none() {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Track is present but disc is missing",
            )];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::TrackRequiresDiscRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(TrackRequiresDiscRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.disc = None;
        assert_eq!(TrackRequiresDiscRule.check(&track).len(), 1);
    }
}
