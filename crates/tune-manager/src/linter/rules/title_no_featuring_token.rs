use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{LintResult, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "title.no-featuring-token",
    description: r#"
        Title must not include featuring tokens; featuring belongs in artist
        field.

        Valid:
        - Song
        - Song (Artist Remix)

        Invalid:
        - Song feat. Singer (featuring token in title)
        - Song ft Singer (featuring token in title)
    "#,
};

static FEAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(feat\.?|featuring|ft\.?)\b").unwrap());

pub struct TitleNoFeaturingTokenRule;

impl Rule for TitleNoFeaturingTokenRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> LintResult {
        let track = &target.track;
        let Some(title) = track.fields.title.as_deref() else {
            return LintResult::Passed;
        };
        if FEAT_RE.is_match(title) {
            return self
                .error("Title should not include featuring token")
                .into();
        }
        LintResult::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::TitleNoFeaturingTokenRule;
    use crate::linter::{Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(
            TitleNoFeaturingTokenRule
                .check(&make_track().into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.title = Some("Song feat. Singer".to_string());
        assert_eq!(
            TitleNoFeaturingTokenRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_case_ft() {
        let mut track = make_track();
        track.fields.title = Some("Song ft Singer".to_string());
        assert_eq!(
            TitleNoFeaturingTokenRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }
}
