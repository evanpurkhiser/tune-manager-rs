use crate::{
    linter::{CheckOutcome, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "album.requires-disc",
    description: r#"
        If album is present, disc must also be present.

        Valid:
        - album=Album, disc=1/1
        - album missing, disc missing

        Invalid:
        - album=Album, disc missing (album track must declare disc)
    "#,
};

pub struct AlbumRequiresDiscRule;

impl Rule for AlbumRequiresDiscRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> CheckOutcome {
        let track = &target.track;
        if track.fields.album.is_some() && track.fields.disc.is_none() {
            return self.error("Album is present but disc is missing").into();
        }
        CheckOutcome::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::AlbumRequiresDiscRule;
    use crate::linter::{Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(
            AlbumRequiresDiscRule
                .check(&make_track().into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.disc = None;
        assert_eq!(
            AlbumRequiresDiscRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }
}
