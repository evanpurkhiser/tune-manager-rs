use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{CheckOutcome, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "artist.feat-standardization",
    description: r#"
        Featuring token in artist field must be canonical Ft.

        Valid:
        - Artist A Ft. Artist B
        - Artist A

        Invalid:
        - Artist A feat. Artist B (use Ft.)
        - Artist A featuring Artist B (use Ft.)
    "#,
    autofix_notes: r#"
        Replaces non-canonical featuring tokens (`feat.`, `featuring`,
        `ft.`, `ft`, case-insensitive) with `Ft.`.
    "#,
};

static FEAT_VARIANT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s(featuring|feat\.?|ft\.?)\s").unwrap());

pub struct ArtistFeatStandardizationRule;

impl Rule for ArtistFeatStandardizationRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> CheckOutcome {
        let track = &target.track;
        let Some(artist) = track.fields.artist.as_deref() else {
            return CheckOutcome::Passed;
        };
        let Some(cap) = FEAT_VARIANT_RE.captures(artist) else {
            return CheckOutcome::Passed;
        };
        let token = cap
            .get(1)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        if token == "ft." {
            return CheckOutcome::Passed;
        }
        self.error("Featuring token should be canonical Ft.")
            .with_fix(|track| {
                if let Some(artist) = track.fields.artist.as_deref() {
                    track.fields.artist = Some(standardize_feat(artist));
                }
            })
            .into()
    }
}

fn standardize_feat(artist: &str) -> String {
    FEAT_VARIANT_RE.replace_all(artist, " Ft. ").to_string()
}

#[cfg(test)]
mod tests {
    use super::ArtistFeatStandardizationRule;
    use crate::linter::{LintTarget, Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.fields.artist = Some("A Ft. B".to_string());
        assert!(
            ArtistFeatStandardizationRule
                .check(&track.into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.artist = Some("A feat. B".to_string());
        assert_eq!(
            ArtistFeatStandardizationRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    fn fixed_artist(input: &str) -> String {
        let mut track = make_track();
        track.fields.artist = Some(input.to_string());
        let mut target: LintTarget = track.into();
        let result = ArtistFeatStandardizationRule.check(&target);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        target.track.fields.artist.unwrap()
    }

    #[test]
    fn fix_feat_dot() {
        assert_eq!(fixed_artist("A feat. B"), "A Ft. B");
    }

    #[test]
    fn fix_featuring() {
        assert_eq!(fixed_artist("A featuring B"), "A Ft. B");
    }

    #[test]
    fn fix_ft_no_dot() {
        assert_eq!(fixed_artist("A ft B"), "A Ft. B");
    }

    #[test]
    fn fix_uppercase() {
        assert_eq!(fixed_artist("A FEAT B"), "A Ft. B");
    }
}
