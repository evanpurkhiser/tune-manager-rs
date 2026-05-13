use crate::{
    linter::{LintResult, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.text-trimmed",
    description: r#"
        Text-valued tag fields must have no leading or trailing whitespace.
        Applies to every string-valued tag field (artist, title, album,
        remixer, publisher, catalog_id, year, genre, key, bpm).

        Valid:
        - artist=Artist
        - bpm=128.5

        Invalid:
        - artist=" Artist" (leading whitespace)
        - title="Title  " (trailing whitespace)
    "#,
    autofix_notes: r#"
        Emits one violation per affected field. Each fix trims that
        field's value.

        Whitespace-only values trim to empty strings — emptiness is
        caught separately by `meta.required-fields-present`.
    "#,
};

#[derive(Copy, Clone)]
struct TrimmableField {
    name: &'static str,
    get: fn(&Track) -> Option<&str>,
    set: fn(&mut Track, String),
}

const FIELDS: &[TrimmableField] = &[
    TrimmableField {
        name: "artist",
        get: |t| t.tags.artist.as_deref(),
        set: |t, v| t.tags.artist = Some(v),
    },
    TrimmableField {
        name: "title",
        get: |t| t.tags.title.as_deref(),
        set: |t, v| t.tags.title = Some(v),
    },
    TrimmableField {
        name: "album",
        get: |t| t.tags.album.as_deref(),
        set: |t, v| t.tags.album = Some(v),
    },
    TrimmableField {
        name: "remixer",
        get: |t| t.tags.remixer.as_deref(),
        set: |t, v| t.tags.remixer = Some(v),
    },
    TrimmableField {
        name: "publisher",
        get: |t| t.tags.publisher.as_deref(),
        set: |t, v| t.tags.publisher = Some(v),
    },
    TrimmableField {
        name: "catalog_id",
        get: |t| t.tags.catalog_id.as_deref(),
        set: |t, v| t.tags.catalog_id = Some(v),
    },
    TrimmableField {
        name: "year",
        get: |t| t.tags.year.as_deref(),
        set: |t, v| t.tags.year = Some(v),
    },
    TrimmableField {
        name: "genre",
        get: |t| t.tags.genre.as_deref(),
        set: |t, v| t.tags.genre = Some(v),
    },
    TrimmableField {
        name: "key",
        get: |t| t.tags.key.as_deref(),
        set: |t, v| t.tags.key = Some(v),
    },
    TrimmableField {
        name: "bpm",
        get: |t| t.tags.bpm.as_deref(),
        set: |t, v| t.tags.bpm = Some(v),
    },
];

pub struct MetaTextTrimmedRule;

impl Rule for MetaTextTrimmedRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        FIELDS
            .iter()
            .filter(|f| {
                let getter = f.get;
                getter(track).is_some_and(|v| v.trim().len() != v.len())
            })
            .map(|f| {
                self.error(format!("`{}` has leading or trailing whitespace", f.name))
                    .with_fix(move |t| {
                        let getter = f.get;
                        let setter = f.set;
                        if let Some(v) = getter(t) {
                            setter(t, v.trim().to_string());
                        }
                    })
            })
            .collect::<Vec<_>>()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::MetaTextTrimmedRule;
    use crate::linter::{Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(MetaTextTrimmedRule.check(&make_track()).is_passed());
    }

    #[test]
    fn fail_leading_space_in_artist() {
        let mut track = make_track();
        track.tags.artist = Some(" Artist".to_string());
        assert_eq!(MetaTextTrimmedRule.check(&track).violations().len(), 1);
    }

    #[test]
    fn fail_trailing_space_in_title() {
        let mut track = make_track();
        track.tags.title = Some("Title ".to_string());
        assert_eq!(MetaTextTrimmedRule.check(&track).violations().len(), 1);
    }

    #[test]
    fn one_violation_per_affected_field() {
        let mut track = make_track();
        track.tags.artist = Some(" Artist ".to_string());
        track.tags.bpm = Some(" 128.5".to_string());
        assert_eq!(MetaTextTrimmedRule.check(&track).violations().len(), 2);
    }

    #[test]
    fn fix_targets_only_violated_field() {
        let mut track = make_track();
        track.tags.artist = Some(" Artist".to_string());
        let original_title = track.tags.title.clone();
        let result = MetaTextTrimmedRule.check(&track);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut track);
        assert_eq!(track.tags.artist.as_deref(), Some("Artist"));
        assert_eq!(track.tags.title, original_title);
    }

    #[test]
    fn fix_trims_both_sides() {
        let mut track = make_track();
        track.tags.bpm = Some("  128.5  ".to_string());
        let result = MetaTextTrimmedRule.check(&track);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut track);
        assert_eq!(track.tags.bpm.as_deref(), Some("128.5"));
    }

    #[test]
    fn whitespace_only_value_trims_to_empty_string() {
        // Whitespace-only is technically "trimmable" — the fix produces
        // an empty string. Separate rules (meta.required-fields-present)
        // are responsible for catching empty values.
        let mut track = make_track();
        track.tags.album = Some("   ".to_string());
        let result = MetaTextTrimmedRule.check(&track);
        assert_eq!(result.violations().len(), 1);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut track);
        assert_eq!(track.tags.album.as_deref(), Some(""));
    }

    #[test]
    fn empty_string_is_not_a_violation() {
        let mut track = make_track();
        track.tags.album = Some(String::new());
        assert!(MetaTextTrimmedRule.check(&track).is_passed());
    }
}
