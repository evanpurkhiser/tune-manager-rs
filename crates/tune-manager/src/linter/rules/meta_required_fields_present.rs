use crate::{
    linter::{LintResult, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.required-fields-present",
    description: r#"
        Required metadata fields (artist, title, media_hash) must be present
        and non-empty.

        `media_hash` is written by tune-manager into the UFID frame during
        ingest and acts as the track's stable identity. A missing value means
        the track has not been ingested (or the frame was stripped) and must
        be backfilled before the track can be reliably tracked.

        Valid:
        - artist=Artist, title=Title, media_hash=<hex>

        Invalid:
        - artist missing
        - title missing
        - media_hash missing
        - any required field set to whitespace-only string
    "#,
};

pub struct MetaRequiredFieldsPresentRule;

impl Rule for MetaRequiredFieldsPresentRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> LintResult {
        let track = &target.track;
        let required = [
            ("artist", track.fields.artist.as_deref()),
            ("title", track.fields.title.as_deref()),
            ("media_hash", track.fields.media_hash.as_deref()),
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
                .check(&make_track().into())
                .is_passed()
        );
    }

    #[test]
    fn fail_missing_artist() {
        let mut track = make_track();
        track.fields.artist = None;
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_missing_title() {
        let mut track = make_track();
        track.fields.title = None;
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_missing_media_hash() {
        let mut track = make_track();
        track.fields.media_hash = None;
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_missing_all() {
        let mut track = make_track();
        track.fields.artist = None;
        track.fields.title = None;
        track.fields.media_hash = None;
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track.into())
                .violations()
                .len(),
            3
        );
    }

    #[test]
    fn fail_whitespace_only() {
        let mut track = make_track();
        track.fields.artist = Some("   ".to_string());
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_empty_string() {
        let mut track = make_track();
        track.fields.title = Some(String::new());
        assert_eq!(
            MetaRequiredFieldsPresentRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }
}
