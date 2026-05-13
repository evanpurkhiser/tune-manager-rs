use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{RuleMetadata, RuleViolation, TrackRule},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "artist.separator-structure",
    description: r#"
        Artist separators must form a clean structure with no malformed
        whitespace or duplicated separators.

        Hygiene:
        - No leading or trailing whitespace or separators.
        - No whitespace before a comma.
        - No consecutive whitespace inside the body.
        - No duplicated separators (`,,`, `& &`, etc.).

        Any mix of canonical separators (`,`, `vs`, `&`) is allowed; choice of
        separator is left to the writer.

        Valid:
        - A & B
        - A vs B
        - A, B, C
        - Technikore vs Dougal & Gammer
        - Aly & Fila, Lostly

        Invalid:
        - A & & B (duplicate separator)
        - A ,  B (bad spacing)

        Not detected here: mixed-type doubled separators between the same
        pair (e.g. `A , vs B`) — those require understanding artist tokens
        rather than just hygiene patterns. Left for a future rule.
    "#,
};

static LEADING_JUNK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[,&\s]+").unwrap());
static TRAILING_JUNK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[,&\s]+$").unwrap());
static SPACE_BEFORE_COMMA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+,").unwrap());
static DOUBLE_SPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s{2,}").unwrap());
static DOUBLED_COMMA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",\s*,").unwrap());
static DOUBLED_AMP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&\s*&").unwrap());

fn is_separator_or_ws(c: char) -> bool {
    c == ',' || c == '&' || c.is_whitespace()
}

// TODO: apply this rule to the `remixer` field too. The cleanest path is a
// shared `ArtistField` selector enum and registering this rule twice.
pub struct ArtistSeparatorStructureRule;

impl TrackRule for ArtistSeparatorStructureRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(artist) = track.tags.artist.as_deref() else {
            return vec![];
        };
        let mut violations = vec![];

        if LEADING_JUNK_RE.is_match(artist) {
            violations.push(
                self.error("Artist has leading separator or whitespace")
                    .with_fix(fix_trim_leading),
            );
        }
        if TRAILING_JUNK_RE.is_match(artist) {
            violations.push(
                self.error("Artist has trailing separator or whitespace")
                    .with_fix(fix_trim_trailing),
            );
        }

        // Inner checks run against the trimmed body so leading/trailing
        // whitespace doesn't surface as both a junk violation AND a
        // consecutive-spaces violation.
        let body = artist.trim_matches(is_separator_or_ws);

        if SPACE_BEFORE_COMMA_RE.is_match(body) {
            violations.push(
                self.error("Artist has whitespace before comma")
                    .with_fix(fix_space_before_comma),
            );
        }
        if DOUBLE_SPACE_RE.is_match(body) {
            violations.push(
                self.error("Artist has consecutive whitespace")
                    .with_fix(fix_collapse_spaces),
            );
        }
        if DOUBLED_COMMA_RE.is_match(body) {
            violations.push(
                self.error("Artist has doubled comma separator")
                    .with_fix(fix_collapse_doubled_comma),
            );
        }
        if DOUBLED_AMP_RE.is_match(body) {
            violations.push(
                self.error("Artist has doubled ampersand separator")
                    .with_fix(fix_collapse_doubled_amp),
            );
        }

        violations
    }
}

fn fix_trim_leading(track: &mut Track) {
    if let Some(a) = track.tags.artist.as_deref() {
        track.tags.artist = Some(a.trim_start_matches(is_separator_or_ws).to_string());
    }
}

fn fix_trim_trailing(track: &mut Track) {
    if let Some(a) = track.tags.artist.as_deref() {
        track.tags.artist = Some(a.trim_end_matches(is_separator_or_ws).to_string());
    }
}

fn fix_space_before_comma(track: &mut Track) {
    if let Some(a) = track.tags.artist.as_deref() {
        track.tags.artist = Some(SPACE_BEFORE_COMMA_RE.replace_all(a, ",").to_string());
    }
}

fn fix_collapse_spaces(track: &mut Track) {
    if let Some(a) = track.tags.artist.as_deref() {
        track.tags.artist = Some(DOUBLE_SPACE_RE.replace_all(a, " ").to_string());
    }
}

fn fix_collapse_doubled_comma(track: &mut Track) {
    if let Some(a) = track.tags.artist.as_deref() {
        track.tags.artist = Some(DOUBLED_COMMA_RE.replace_all(a, ",").to_string());
    }
}

fn fix_collapse_doubled_amp(track: &mut Track) {
    if let Some(a) = track.tags.artist.as_deref() {
        track.tags.artist = Some(DOUBLED_AMP_RE.replace_all(a, "&").to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::ArtistSeparatorStructureRule;
    use crate::linter::{TrackRule, test_utils::make_track};

    fn check_with(artist: &str) -> Vec<crate::linter::RuleViolation> {
        let mut track = make_track();
        track.tags.artist = Some(artist.to_string());
        ArtistSeparatorStructureRule.check(&track)
    }

    fn fix_all(artist: &str) -> String {
        let mut track = make_track();
        track.tags.artist = Some(artist.to_string());
        for v in ArtistSeparatorStructureRule.check(&track) {
            if let Some(fix) = &v.fix {
                fix.apply(&mut track);
            }
        }
        track.tags.artist.unwrap()
    }

    #[test]
    fn ok_single_artist() {
        assert!(check_with("A").is_empty());
    }

    #[test]
    fn ok_two_with_ampersand() {
        assert!(check_with("A & B").is_empty());
    }

    #[test]
    fn ok_two_with_vs() {
        assert!(check_with("A vs B").is_empty());
    }

    #[test]
    fn ok_two_with_comma() {
        assert!(check_with("A, B").is_empty());
    }

    #[test]
    fn ok_three_with_commas() {
        assert!(check_with("A, B, C").is_empty());
    }

    #[test]
    fn ok_three_mixed_amp_and_comma() {
        assert!(check_with("Aly & Fila, Lostly").is_empty());
    }

    #[test]
    fn ok_three_mixed_vs_and_amp() {
        assert!(check_with("Technikore vs Dougal & Gammer").is_empty());
    }

    #[test]
    fn fail_double_space() {
        assert_eq!(check_with("A  B").len(), 1);
    }

    #[test]
    fn fail_space_before_comma() {
        assert_eq!(check_with("A , B").len(), 1);
    }

    #[test]
    fn fail_doubled_ampersand() {
        assert_eq!(check_with("A & & B").len(), 1);
    }

    #[test]
    fn fail_doubled_comma() {
        // `A,, B` trips just doubled-comma. The fix collapses to one.
        assert_eq!(check_with("A,, B").len(), 1);
    }

    #[test]
    fn fail_doubled_comma_with_leading_space() {
        // `A ,, B` trips BOTH space-before-comma AND doubled-comma.
        // Per-cause split surfaces both; their fixes converge under
        // sequential application.
        assert_eq!(check_with("A ,, B").len(), 2);
    }

    #[test]
    fn fail_leading_separator() {
        assert_eq!(check_with(", A & B").len(), 1);
    }

    #[test]
    fn fail_trailing_separator() {
        assert_eq!(check_with("A & B,").len(), 1);
    }

    #[test]
    fn leading_whitespace_does_not_count_as_double_space() {
        // Should emit ONLY a leading-junk violation, not also a
        // consecutive-whitespace one.
        assert_eq!(check_with("  A & B").len(), 1);
    }

    #[test]
    fn multiple_distinct_issues_emit_multiple_violations() {
        // Leading junk + doubled ampersand inside.
        let violations = check_with("  A & & B");
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn fix_trim_leading_separator() {
        assert_eq!(fix_all(", A & B"), "A & B");
    }

    #[test]
    fn fix_trim_trailing_separator() {
        assert_eq!(fix_all("A & B,"), "A & B");
    }

    #[test]
    fn fix_trim_leading_whitespace() {
        assert_eq!(fix_all("  A & B"), "A & B");
    }

    #[test]
    fn fix_collapse_double_space() {
        assert_eq!(fix_all("A  B"), "A B");
    }

    #[test]
    fn fix_space_before_comma() {
        assert_eq!(fix_all("A , B"), "A, B");
    }

    #[test]
    fn fix_doubled_ampersand() {
        assert_eq!(fix_all("A & & B"), "A & B");
    }

    #[test]
    fn fix_doubled_comma() {
        assert_eq!(fix_all("A ,, B"), "A, B");
    }

    #[test]
    fn fix_combined_leading_and_inner() {
        // One pass should handle both fixes since the engine runs them
        // sequentially against the same `track`.
        assert_eq!(fix_all("  A & & B"), "A & B");
    }
}
