use crate::{
    linter::{CheckOutcome, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "track.requires-disc",
    description: r#"
        If track is present, disc must also be present.

        Valid:
        - track=01/10, disc=1/1
        - track missing, disc missing

        Invalid:
        - track=01/10, disc missing (track count requires disc context)
    "#,
};

pub struct TrackRequiresDiscRule;

impl Rule for TrackRequiresDiscRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> CheckOutcome {
        let track = &target.track;
        if track.fields.track.is_some() && track.fields.disc.is_none() {
            return self.error("Track is present but disc is missing").into();
        }
        CheckOutcome::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::TrackRequiresDiscRule;
    use crate::linter::{Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(
            TrackRequiresDiscRule
                .check(&make_track().into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.disc = None;
        assert_eq!(
            TrackRequiresDiscRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }
}
