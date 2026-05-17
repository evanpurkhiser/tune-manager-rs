use crate::{
    linter::{LintResult, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "path.matches-canonical",
    description: r#"
        Path must exactly match the canonical path generated from track tags.

        Valid:
        - Publisher/[RLS] Album/01. [10A] Artist - Title.mp3
        - Publisher/[+singles]/[10A] Artist - Title.mp3

        Invalid:
        - Publisher/[RLS] Album/99. [10A] Artist - Title.mp3 (wrong track number)
        - WrongPublisher/[RLS] Album/01. [10A] Artist - Title.mp3 (non-canonical directory)
    "#,
};

pub struct PathMatchesCanonicalRule;

impl Rule for PathMatchesCanonicalRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> LintResult {
        let track = &target.track;
        let canonical = track.cononical_path();
        let actual = &track.file.file_path;

        if actual.ends_with(&canonical) {
            return LintResult::Passed;
        }

        self.error(format!(
            "Path does not match canonical path: {}",
            canonical.display()
        ))
        .into()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::linter::{Rule, test_utils::make_track};

    use super::PathMatchesCanonicalRule;

    #[test]
    fn ok_case() {
        let track = make_track();
        assert!(PathMatchesCanonicalRule.check(&track.into()).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.file.file_path =
            PathBuf::from("Publisher/[RLS] Album/Disc 2/99. [10A] Artist - Title.MP3");
        assert_eq!(
            PathMatchesCanonicalRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }
}
