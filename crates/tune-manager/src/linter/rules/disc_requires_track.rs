use crate::{
    linter::{LintResult, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "disc.requires-track",
    description: r#"
        If disc is present, track must also be present.

        Valid:
        - disc=1/2, track=01/10
        - disc missing, track missing

        Invalid:
        - disc=1/2, track missing (disc context without track index)
    "#,
};

pub struct DiscRequiresTrackRule;

impl Rule for DiscRequiresTrackRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        if track.tags.disc.is_some() && track.tags.track.is_none() {
            return self.error("Disc is present but track is missing").into();
        }
        LintResult::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::DiscRequiresTrackRule;
    use crate::linter::{Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(DiscRequiresTrackRule.check(&make_track()).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.track = None;
        assert_eq!(DiscRequiresTrackRule.check(&track).violations().len(), 1);
    }
}
