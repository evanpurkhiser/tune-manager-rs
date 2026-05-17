use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{CheckOutcome, LintTarget, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.remixer-title-consistency",
    description: r#"
        Remixer field and title remix note must agree.

        Valid:
        - title=Song (Remixer Remix), remixer=Remixer
        - title=Song, remixer missing

        Invalid:
        - title=Song (Other Remix), remixer=Remixer (name mismatch)
        - title=Song (Remixer Remix), remixer missing (missing remixer field)
    "#,
    autofix_notes: r#"
        Treats the title as authoritative — extracts the artist from the
        title's `(Artist Remix)` group and writes it into the remixer
        field. Fires for both "remixer empty" and "remixer mismatched"
        cases.

        Does NOT fix the opposite case (remixer set, title has no remix
        note). Writing a remix suffix into the title requires deciding
        placement and style, which isn't clear cut.
    "#,
};

static REMIX_ARTIST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\(([^\)]+?)\s+remix\)").unwrap());

pub struct MetaRemixerTitleConsistencyRule;

impl Rule for MetaRemixerTitleConsistencyRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> CheckOutcome {
        let track = &target.track;
        let title = track.fields.title.as_deref().unwrap_or_default();
        let remixer = track
            .fields
            .remixer
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let title_artist = REMIX_ARTIST_RE
            .captures(title)
            .map(|c| c[1].trim().to_string());

        match (remixer, title_artist.as_deref()) {
            (None, Some(_)) => self
                .error("Title has remix signal but remixer field is empty")
                .with_fix(extract_remixer_from_title)
                .into(),
            (Some(_), None) => self
                .error("Remixer is set but title has no remix signal")
                .into(),
            (Some(remixer), Some(artist)) if remixer != artist => self
                .error("Remixer field does not match title remix note")
                .with_fix(extract_remixer_from_title)
                .into(),
            _ => CheckOutcome::Passed,
        }
    }
}

fn extract_remixer_from_title(track: &mut Track) {
    let Some(title) = track.fields.title.as_deref() else {
        return;
    };
    if let Some(caps) = REMIX_ARTIST_RE.captures(title) {
        track.fields.remixer = Some(caps[1].trim().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::MetaRemixerTitleConsistencyRule;
    use crate::linter::{LintTarget, Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.fields.title = Some("Song (Remixer Remix)".to_string());
        track.fields.remixer = Some("Remixer".to_string());
        assert!(
            MetaRemixerTitleConsistencyRule
                .check(&track.into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.title = Some("Song (Other Remix)".to_string());
        track.fields.remixer = Some("Remixer".to_string());
        assert_eq!(
            MetaRemixerTitleConsistencyRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_inverse_case() {
        let mut track = make_track();
        track.fields.title = Some("Song".to_string());
        track.fields.remixer = Some("Remixer".to_string());
        assert_eq!(
            MetaRemixerTitleConsistencyRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn ignores_non_remix_mix_token() {
        let mut track = make_track();
        track.fields.title = Some("Song (Extended Mix)".to_string());
        track.fields.remixer = None;
        assert!(
            MetaRemixerTitleConsistencyRule
                .check(&track.into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case_difference() {
        let mut track = make_track();
        track.fields.title = Some("Song (REMIXER Remix)".to_string());
        track.fields.remixer = Some("Remixer".to_string());
        assert_eq!(
            MetaRemixerTitleConsistencyRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fix_extracts_artist_when_remixer_empty() {
        let mut track = make_track();
        track.fields.title = Some("Song (Remixer Remix)".to_string());
        track.fields.remixer = None;
        let mut target: LintTarget = track.into();
        let result = MetaRemixerTitleConsistencyRule.check(&target);
        assert_eq!(result.violations().len(), 1);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        assert_eq!(target.track.fields.remixer.as_deref(), Some("Remixer"));
    }

    #[test]
    fn fix_overwrites_mismatched_remixer() {
        let mut track = make_track();
        track.fields.title = Some("Song (Other Remix)".to_string());
        track.fields.remixer = Some("Remixer".to_string());
        let mut target: LintTarget = track.into();
        let result = MetaRemixerTitleConsistencyRule.check(&target);
        assert_eq!(result.violations().len(), 1);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        assert_eq!(target.track.fields.remixer.as_deref(), Some("Other"));
    }

    #[test]
    fn no_fix_when_title_has_no_remix_note() {
        // When remixer is set but title has no remix note we don't autofix:
        // adding a remix note into the title is the ambiguous direction.
        let mut track = make_track();
        track.fields.title = Some("Song".to_string());
        track.fields.remixer = Some("Remixer".to_string());
        let result = MetaRemixerTitleConsistencyRule.check(&track.into());
        assert_eq!(result.violations().len(), 1);
        assert!(result.violations()[0].fix.is_none());
    }

    #[test]
    fn fix_extracts_multi_word_artist() {
        let mut track = make_track();
        track.fields.title = Some("Song (Some Artist Remix)".to_string());
        track.fields.remixer = None;
        let mut target: LintTarget = track.into();
        let result = MetaRemixerTitleConsistencyRule.check(&target);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        assert_eq!(target.track.fields.remixer.as_deref(), Some("Some Artist"));
    }
}
