use crate::{
    linter::{LintResult, LintTarget, Rule, RuleMetadata, TextField},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.disallowed-characters",
    description: r#"
        Text metadata must not contain disallowed characters. Currently
        covers smart quotes.

        Valid:
        - Don't Stop
        - Artist "Name"

        Invalid:
        - Don’t Stop (contains curly apostrophe)
        - Artist “Name” (contains curly quotes)
    "#,
    autofix_notes: r#"
        Replaces disallowed characters in every affected text field:
        - `“` `”` → `"`
        - `‘` `’` `´` `` ` `` → `'`

        Emits one violation per affected field so each fix is scoped to
        the field that triggered it.
    "#,
};

fn replace_disallowed(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '“' | '”' => '"',
            '‘' | '’' | '´' | '`' => '\'',
            c => c,
        })
        .collect()
}

fn has_disallowed(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c, '“' | '”' | '‘' | '’' | '´' | '`'))
}

pub struct MetaDisallowedCharactersRule;

impl Rule for MetaDisallowedCharactersRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> LintResult {
        let track = &target.track;
        TextField::ALL
            .into_iter()
            .filter(|field| field.get(track).is_some_and(has_disallowed))
            .map(|field| {
                self.error(format!("Disallowed characters in `{}`", field.name()))
                    .with_fix(move |track| {
                        if let Some(value) = field.get(track) {
                            field.set(track, replace_disallowed(value));
                        }
                    })
            })
            .collect::<Vec<_>>()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::MetaDisallowedCharactersRule;
    use crate::linter::{LintTarget, Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(
            MetaDisallowedCharactersRule
                .check(&make_track().into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.title = Some("Don’t Stop".to_string());
        assert_eq!(
            MetaDisallowedCharactersRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }

    #[test]
    fn one_violation_per_affected_field() {
        let mut track = make_track();
        track.fields.title = Some("Don’t Stop".to_string());
        track.fields.album = Some("“Greatest Hits”".to_string());
        assert_eq!(
            MetaDisallowedCharactersRule
                .check(&track.into())
                .violations()
                .len(),
            2
        );
    }

    #[test]
    fn fix_targets_only_the_violation_field() {
        let mut track = make_track();
        track.fields.title = Some("Don’t Stop".to_string());
        track.fields.album = Some("Album".to_string());
        let mut target: LintTarget = track.into();
        let result = MetaDisallowedCharactersRule.check(&target);
        assert_eq!(result.violations().len(), 1);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        assert_eq!(target.track.fields.title.as_deref(), Some("Don't Stop"));
        assert_eq!(target.track.fields.album.as_deref(), Some("Album"));
    }

    #[test]
    fn fix_double_smart_quotes() {
        let mut track = make_track();
        track.fields.title = Some("“Hello”".to_string());
        let mut target: LintTarget = track.into();
        let result = MetaDisallowedCharactersRule.check(&target);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        assert_eq!(target.track.fields.title.as_deref(), Some(r#""Hello""#));
    }

    #[test]
    fn fix_backtick_and_acute() {
        let mut track = make_track();
        track.fields.title = Some("It`s ´ok´".to_string());
        let mut target: LintTarget = track.into();
        let result = MetaDisallowedCharactersRule.check(&target);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut target.track);
        assert_eq!(target.track.fields.title.as_deref(), Some("It's 'ok'"));
    }

    #[test]
    fn fix_each_field_when_multiple_violate() {
        let mut track = make_track();
        track.fields.title = Some("Don’t Stop".to_string());
        track.fields.artist = Some("A’B".to_string());
        let mut target: LintTarget = track.into();
        let result = MetaDisallowedCharactersRule.check(&target);
        for v in result.violations() {
            v.fix.as_ref().unwrap().apply(&mut target.track);
        }
        assert_eq!(target.track.fields.title.as_deref(), Some("Don't Stop"));
        assert_eq!(target.track.fields.artist.as_deref(), Some("A'B"));
    }
}
