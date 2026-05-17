use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{CheckOutcome, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "artist.separator-standardization",
    description: r#"
        Artist collaboration separators must use canonical tokens.

        Valid:
        - A & B
        - A vs B

        Invalid:
        - A and B (use &)
        - A vs. B (use vs without period)
        - A versus B (use vs)
    "#,
    autofix_notes: r#"
        Rewrites non-canonical tokens to their canonical form:
        - `and` → `&`
        - `vs.` → `vs`
        - `versus` → `vs`
    "#,
};

static NON_CANON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s(and|vs\.|versus)\s").unwrap());

// TODO: apply this rule to the `remixer` field too. The cleanest path is a
// shared `ArtistField` selector enum and registering this rule twice.
pub struct ArtistSeparatorStandardizationRule;

impl Rule for ArtistSeparatorStandardizationRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> CheckOutcome {
        let track = &target.track;
        let Some(artist) = track.fields.artist.as_deref() else {
            return CheckOutcome::Passed;
        };
        if !NON_CANON_RE.is_match(artist) {
            return CheckOutcome::Passed;
        }
        self.error("Artist connectors are not canonical")
            .with_fix(|track| {
                if let Some(artist) = track.fields.artist.as_deref() {
                    track.fields.artist = Some(standardize_separators(artist));
                }
            })
            .into()
    }
}

fn standardize_separators(artist: &str) -> String {
    NON_CANON_RE
        .replace_all(artist, |caps: &regex::Captures| {
            match caps[1].to_ascii_lowercase().as_str() {
                "and" => " & ".to_string(),
                "vs." | "versus" => " vs ".to_string(),
                _ => caps[0].to_string(),
            }
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::ArtistSeparatorStandardizationRule;
    use crate::linter::{LintTarget, Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.fields.artist = Some("A & B".to_string());
        assert!(
            ArtistSeparatorStandardizationRule
                .check(&track.into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.artist = Some("A and B".to_string());
        assert_eq!(
            ArtistSeparatorStandardizationRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fail_versus() {
        let mut track = make_track();
        track.fields.artist = Some("A versus B".to_string());
        assert_eq!(
            ArtistSeparatorStandardizationRule
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
        let result = ArtistSeparatorStandardizationRule.check(&target);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        target.track.fields.artist.unwrap()
    }

    #[test]
    fn fix_and_to_ampersand() {
        assert_eq!(fixed_artist("A and B"), "A & B");
    }

    #[test]
    fn fix_vs_dot_to_vs() {
        assert_eq!(fixed_artist("A vs. B"), "A vs B");
    }

    #[test]
    fn fix_versus_to_vs() {
        assert_eq!(fixed_artist("A versus B"), "A vs B");
    }

    #[test]
    fn fix_mixed_in_one_pass() {
        assert_eq!(fixed_artist("A and B versus C"), "A & B vs C");
    }
}
