use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
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
};

#[derive(Copy, Clone)]
enum TextField {
    Artist,
    Title,
    Album,
    Remixer,
    Publisher,
}

impl TextField {
    fn name(self) -> &'static str {
        match self {
            Self::Artist => "artist",
            Self::Title => "title",
            Self::Album => "album",
            Self::Remixer => "remixer",
            Self::Publisher => "publisher",
        }
    }

    fn get(self, track: &Track) -> Option<&str> {
        match self {
            Self::Artist => track.tags.artist.as_deref(),
            Self::Title => track.tags.title.as_deref(),
            Self::Album => track.tags.album.as_deref(),
            Self::Remixer => track.tags.remixer.as_deref(),
            Self::Publisher => track.tags.publisher.as_deref(),
        }
    }

    fn set(self, track: &mut Track, value: String) {
        match self {
            Self::Artist => track.tags.artist = Some(value),
            Self::Title => track.tags.title = Some(value),
            Self::Album => track.tags.album = Some(value),
            Self::Remixer => track.tags.remixer = Some(value),
            Self::Publisher => track.tags.publisher = Some(value),
        }
    }
}

const FIELDS: [TextField; 5] = [
    TextField::Artist,
    TextField::Title,
    TextField::Album,
    TextField::Remixer,
    TextField::Publisher,
];

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
        FIELDS
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
    use crate::rules::{TrackRule, test_utils::make_track};

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
