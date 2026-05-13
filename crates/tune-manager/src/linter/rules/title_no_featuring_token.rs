use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{LintResult, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
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

    fn check(&self, track: &Track) -> LintResult {
        let Some(title) = track.tags.title.as_deref() else {
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
        assert!(TitleNoFeaturingTokenRule.check(&make_track()).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.title = Some("Song feat. Singer".to_string());
        assert_eq!(
            TitleNoFeaturingTokenRule.check(&track).violations().len(),
            1
        );
    }

    #[test]
    fn fail_case_ft() {
        let mut track = make_track();
        track.tags.title = Some("Song ft Singer".to_string());
        assert_eq!(
            TitleNoFeaturingTokenRule.check(&track).violations().len(),
            1
        );
    }
}
