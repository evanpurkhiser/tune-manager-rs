use crate::{
    rule_metadata,
    linter::{RuleMetadata, RuleViolation, TextField, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.no-smart-quotes",
    description: r#"
        Text metadata must not contain smart quote characters.

        Valid:
        - Don't Stop
        - Artist "Name"

        Invalid:
        - Don’t Stop (contains curly apostrophe)
        - Artist “Name” (contains curly quotes)
    "#,
    autofix_notes: r#"
        Replaces smart quotes in every affected text field:
        - `“` `”` → `"`
        - `‘` `’` `´` `` ` `` → `'`

        Emits one violation per affected field so each fix is scoped to
        the field that triggered it.
    "#,
};

fn replace_smart_quotes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '“' | '”' => '"',
            '‘' | '’' | '´' | '`' => '\'',
            c => c,
        })
        .collect()
}

fn has_smart_quotes(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c, '“' | '”' | '‘' | '’' | '´' | '`'))
}

pub struct MetaNoSmartQuotesRule;

impl TrackRule for MetaNoSmartQuotesRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        TextField::ALL
            .into_iter()
            .filter(|field| field.get(track).is_some_and(has_smart_quotes))
            .map(|field| {
                self.error(format!("Smart quotes in `{}`", field.name()))
                    .with_fix(move |track| {
                        if let Some(value) = field.get(track) {
                            field.set(track, replace_smart_quotes(value));
                        }
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::MetaNoSmartQuotesRule;
    use crate::linter::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(MetaNoSmartQuotesRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.title = Some("Don’t Stop".to_string());
        assert_eq!(MetaNoSmartQuotesRule.check(&track).len(), 1);
    }

    #[test]
    fn one_violation_per_affected_field() {
        let mut track = make_track();
        track.tags.title = Some("Don’t Stop".to_string());
        track.tags.album = Some("“Greatest Hits”".to_string());
        assert_eq!(MetaNoSmartQuotesRule.check(&track).len(), 2);
    }

    #[test]
    fn fix_targets_only_the_violation_field() {
        let mut track = make_track();
        track.tags.title = Some("Don’t Stop".to_string());
        track.tags.album = Some("Album".to_string());
        let violations = MetaNoSmartQuotesRule.check(&track);
        assert_eq!(violations.len(), 1);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        assert_eq!(track.tags.title.as_deref(), Some("Don't Stop"));
        assert_eq!(track.tags.album.as_deref(), Some("Album"));
    }

    #[test]
    fn fix_double_smart_quotes() {
        let mut track = make_track();
        track.tags.title = Some("“Hello”".to_string());
        let violations = MetaNoSmartQuotesRule.check(&track);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        assert_eq!(track.tags.title.as_deref(), Some(r#""Hello""#));
    }

    #[test]
    fn fix_backtick_and_acute() {
        let mut track = make_track();
        track.tags.title = Some("It`s ´ok´".to_string());
        let violations = MetaNoSmartQuotesRule.check(&track);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        assert_eq!(track.tags.title.as_deref(), Some("It's 'ok'"));
    }

    #[test]
    fn fix_each_field_when_multiple_violate() {
        let mut track = make_track();
        track.tags.title = Some("Don’t Stop".to_string());
        track.tags.artist = Some("A’B".to_string());
        let violations = MetaNoSmartQuotesRule.check(&track);
        for v in &violations {
            v.fix.as_ref().unwrap().apply(&mut track);
        }
        assert_eq!(track.tags.title.as_deref(), Some("Don't Stop"));
        assert_eq!(track.tags.artist.as_deref(), Some("A'B"));
    }
}
