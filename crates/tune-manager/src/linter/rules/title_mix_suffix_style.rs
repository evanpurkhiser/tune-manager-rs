use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{LintResult, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "title.mix-suffix-style",
    description: r#"
        Mix/edit/version suffix in title must use canonical capitalization.

        Valid:
        - Song (Artist Remix)
        - Song (Producer Edit)

        Invalid:
        - Song (artist remix) (non-canonical suffix style)
        - Song (artist version) (non-canonical suffix style)
    "#,
    autofix_notes: r#"
        Title-cases the mix/edit/version keyword inside the parens. The
        artist name portion is left untouched so intentionally lowercase
        artists like `deadmau5` are preserved.
    "#,
};

static FUZZY_MIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\(([^\)]*?)\s(remix|edit|mix|version)\)").unwrap());

pub struct TitleMixSuffixStyleRule;

impl Rule for TitleMixSuffixStyleRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> LintResult {
        let track = &target.track;
        let Some(title) = track.fields.title.as_deref() else {
            return LintResult::Passed;
        };
        let any_non_canonical = FUZZY_MIX_RE
            .captures_iter(title)
            .any(|caps| caps[2] != title_case(&caps[2]));
        if !any_non_canonical {
            return LintResult::Passed;
        }
        self.error("Title mix suffix is not canonical")
            .with_fix(|track| {
                if let Some(title) = track.fields.title.as_deref() {
                    track.fields.title = Some(canonicalize_mix_suffix(title));
                }
            })
            .into()
    }
}

fn canonicalize_mix_suffix(title: &str) -> String {
    FUZZY_MIX_RE
        .replace_all(title, |caps: &regex::Captures| {
            let prefix = &caps[1];
            let keyword = title_case(&caps[2]);
            format!("({prefix} {keyword})")
        })
        .to_string()
}

// REVIEW: Let's pull in a crate for this
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::TitleMixSuffixStyleRule;
    use crate::linter::{LintTarget, Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.fields.title = Some("Song (Artist Remix)".to_string());
        assert!(TitleMixSuffixStyleRule.check(&track.into()).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.title = Some("Song (artist remix)".to_string());
        assert_eq!(
            TitleMixSuffixStyleRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    fn fixed_title(input: &str) -> String {
        let mut track = make_track();
        track.fields.title = Some(input.to_string());
        let mut target: LintTarget = track.into();
        let result = TitleMixSuffixStyleRule.check(&target);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        target.track.fields.title.unwrap()
    }

    #[test]
    fn fix_lowercase_remix() {
        assert_eq!(fixed_title("Song (artist remix)"), "Song (artist Remix)");
    }

    #[test]
    fn fix_uppercase_keyword() {
        assert_eq!(fixed_title("Song (artist REMIX)"), "Song (artist Remix)");
    }

    #[test]
    fn fix_edit_keyword() {
        assert_eq!(fixed_title("Song (artist edit)"), "Song (artist Edit)");
    }

    #[test]
    fn fix_preserves_artist_casing() {
        assert_eq!(
            fixed_title("Song (deadmau5 remix)"),
            "Song (deadmau5 Remix)"
        );
    }

    #[test]
    fn fail_when_one_group_canonical_and_one_not() {
        let mut track = make_track();
        track.fields.title = Some("Song (artist remix) (Foo Edit)".to_string());
        assert_eq!(
            TitleMixSuffixStyleRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn fix_only_non_canonical_groups() {
        assert_eq!(
            fixed_title("Song (artist remix) (Foo Edit)"),
            "Song (artist Remix) (Foo Edit)"
        );
    }
}
