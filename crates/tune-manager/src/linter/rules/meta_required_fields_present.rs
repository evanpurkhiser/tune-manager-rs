use crate::{
    linter::{LintResult, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.required-fields-present",
    description: r#"
        Required metadata fields (artist, title) must be present and
        non-empty.

        Valid:
        - artist=Artist, title=Title

        Invalid:
        - artist missing
        - title missing
        - artist or title set to whitespace-only string
    "#,
};

pub struct MetaRequiredFieldsPresentRule;

impl Rule for MetaRequiredFieldsPresentRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        let required = [
            ("artist", track.tags.artist.as_deref()),
            ("title", track.tags.title.as_deref()),
        ];

        required
            .into_iter()
            .filter(|(_, value)| !value.is_some_and(|v| !v.trim().is_empty()))
            .map(|(name, _)| self.error(format!("Required field `{name}` is missing or empty")))
            .collect::<Vec<_>>()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::MetaRequiredFieldsPresentRule;
    use crate::linter::{Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(
            MetaRequiredFieldsPresentRule
                .check(&make_track())
                .is_passed()
        );
    }

    #[test]
    fn fail_missing_artist() {
        let mut track = make_track();
        track.tags.artist = None;
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track)
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_missing_title() {
        let mut track = make_track();
        track.tags.title = None;
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track)
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_missing_both() {
        let mut track = make_track();
        track.tags.artist = None;
        track.tags.title = None;
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track)
                .violations()
                .len(),
            2
        );
    }

    #[test]
    fn fail_whitespace_only() {
        let mut track = make_track();
        track.tags.artist = Some("   ".to_string());
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track)
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_empty_string() {
        let mut track = make_track();
        track.tags.title = Some(String::new());
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track)
                .violations()
                .len(),
            1
        );
    }
}
