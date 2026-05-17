use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{LintResult, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "key.canonical-camelot",
    description: r#"
        Key must use canonical Camelot notation, zero-padded.

        Valid:
        - 01A..12A (minor)
        - 01B..12B (major)

        Invalid:
        - 1A (not zero-padded)
        - 01a (suffix must be uppercase)
        - Am (musical notation, not Camelot)
        - 1d (OpenKey notation, not Camelot)
    "#,
    autofix_notes: r#"
        Recognizes and converts these notations into canonical Camelot:
        - Non-canonical Camelot: `1A`, `1a`, `01a`.
        - OpenKey: `1d`/`1m` … `12d`/`12m`.
        - Standard musical: `C`, `Am`, `F#`, `Bbm`. Sharps are accepted as
          aliases of the equivalent flat (`F#` = `Gb`, `C#` = `Db`, etc.).

        Anything outside these notations is reported but left unfixed.
    "#,
};

static CANONICAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(0[1-9]|1[0-2])[AB]$").unwrap());

pub struct KeyCanonicalCamelotRule;

impl Rule for KeyCanonicalCamelotRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        let Some(key) = track.fields.key.as_deref() else {
            return LintResult::Passed;
        };
        let key = key.trim();

        if CANONICAL_RE.is_match(key) {
            return LintResult::Passed;
        }

        if let Some(canonical) = parse_camelot(key) {
            return self
                .error(format!(
                    "Key `{key}` is non-canonical Camelot; should be `{canonical}`"
                ))
                .with_fix(fix_to_canonical_camelot)
                .into();
        }
        if let Some(canonical) = parse_openkey(key) {
            return self
                .error(format!(
                    "Key `{key}` is OpenKey notation; canonical Camelot is `{canonical}`"
                ))
                .with_fix(fix_to_canonical_camelot)
                .into();
        }
        if let Some(canonical) = parse_musical(key) {
            return self
                .error(format!(
                    "Key `{key}` is musical notation; canonical Camelot is `{canonical}`"
                ))
                .with_fix(fix_to_canonical_camelot)
                .into();
        }

        self.error(format!("Key `{key}` is not in a recognized notation"))
            .into()
    }
}

fn fix_to_canonical_camelot(track: &mut Track) {
    let Some(key) = track.fields.key.as_deref() else {
        return;
    };
    let trimmed = key.trim();
    let canonical = parse_camelot(trimmed)
        .or_else(|| parse_openkey(trimmed))
        .or_else(|| parse_musical(trimmed));
    if let Some(c) = canonical {
        track.fields.key = Some(c);
    }
}

fn parse_camelot(s: &str) -> Option<String> {
    let upper = s.to_ascii_uppercase();
    let suffix = upper.chars().last()?;
    if suffix != 'A' && suffix != 'B' {
        return None;
    }
    let num: u32 = upper[..upper.len() - 1].parse().ok()?;
    if !(1..=12).contains(&num) {
        return None;
    }
    Some(format!("{num:02}{suffix}"))
}

fn parse_openkey(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();
    let suffix = lower.chars().last()?;
    if suffix != 'd' && suffix != 'm' {
        return None;
    }
    let num: u32 = lower[..lower.len() - 1].parse().ok()?;
    if !(1..=12).contains(&num) {
        return None;
    }
    let camelot_num = ((num - 1 + 7) % 12) + 1;
    let letter = if suffix == 'd' { 'B' } else { 'A' };
    Some(format!("{camelot_num:02}{letter}"))
}

fn parse_musical(s: &str) -> Option<String> {
    if let Some(stripped) = s.strip_suffix('m') {
        let num = musical_minor_to_number(stripped)?;
        Some(format!("{num:02}A"))
    } else {
        let num = musical_major_to_number(s)?;
        Some(format!("{num:02}B"))
    }
}

fn musical_major_to_number(note: &str) -> Option<u32> {
    match note {
        "C" => Some(8),
        "G" => Some(9),
        "D" => Some(10),
        "A" => Some(11),
        "E" => Some(12),
        "B" => Some(1),
        "Gb" | "F#" => Some(2),
        "Db" | "C#" => Some(3),
        "Ab" | "G#" => Some(4),
        "Eb" | "D#" => Some(5),
        "Bb" | "A#" => Some(6),
        "F" => Some(7),
        _ => None,
    }
}

fn musical_minor_to_number(note: &str) -> Option<u32> {
    match note {
        "A" => Some(8),
        "E" => Some(9),
        "B" => Some(10),
        "Gb" | "F#" => Some(11),
        "Db" | "C#" => Some(12),
        "Ab" | "G#" => Some(1),
        "Eb" | "D#" => Some(2),
        "Bb" | "A#" => Some(3),
        "F" => Some(4),
        "C" => Some(5),
        "G" => Some(6),
        "D" => Some(7),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::KeyCanonicalCamelotRule;
    use crate::linter::{Rule, test_utils::make_track};

    fn check_with(key: &str) -> crate::linter::LintResult {
        let mut track = make_track();
        track.fields.key = Some(key.to_string());
        KeyCanonicalCamelotRule.check(&track)
    }

    fn fixed(key: &str) -> String {
        let mut track = make_track();
        track.fields.key = Some(key.to_string());
        let result = KeyCanonicalCamelotRule.check(&track);
        result.violations()[0]
            .fix
            .as_ref()
            .unwrap()
            .apply(&mut track);
        track.fields.key.unwrap()
    }

    #[test]
    fn ok_canonical() {
        assert!(check_with("01A").is_passed());
        assert!(check_with("12B").is_passed());
        assert!(check_with("08A").is_passed());
    }

    #[test]
    fn fail_unpadded_camelot() {
        assert_eq!(check_with("1A").violations().len(), 1);
    }

    #[test]
    fn fail_lowercase_suffix() {
        assert_eq!(check_with("01a").violations().len(), 1);
    }

    #[test]
    fn fail_unrecognized_no_fix() {
        let result = check_with("xyz");
        assert_eq!(result.violations().len(), 1);
        assert!(result.violations()[0].fix.is_none());
    }

    #[test]
    fn fix_camelot_zero_pads() {
        assert_eq!(fixed("1A"), "01A");
        assert_eq!(fixed("9b"), "09B");
        assert_eq!(fixed("12a"), "12A");
    }

    #[test]
    fn fix_camelot_normalizes_case() {
        assert_eq!(fixed("01a"), "01A");
        assert_eq!(fixed("12b"), "12B");
    }

    #[test]
    fn fix_openkey_major() {
        assert_eq!(fixed("1d"), "08B");
        assert_eq!(fixed("12d"), "07B");
        assert_eq!(fixed("6d"), "01B");
    }

    #[test]
    fn fix_openkey_minor() {
        assert_eq!(fixed("1m"), "08A");
        assert_eq!(fixed("12m"), "07A");
        assert_eq!(fixed("9m"), "04A");
    }

    #[test]
    fn fix_musical_major_naturals() {
        assert_eq!(fixed("C"), "08B");
        assert_eq!(fixed("G"), "09B");
        assert_eq!(fixed("D"), "10B");
        assert_eq!(fixed("A"), "11B");
        assert_eq!(fixed("E"), "12B");
        assert_eq!(fixed("B"), "01B");
        assert_eq!(fixed("F"), "07B");
    }

    #[test]
    fn fix_musical_major_flats() {
        assert_eq!(fixed("Gb"), "02B");
        assert_eq!(fixed("Db"), "03B");
        assert_eq!(fixed("Ab"), "04B");
        assert_eq!(fixed("Eb"), "05B");
        assert_eq!(fixed("Bb"), "06B");
    }

    #[test]
    fn fix_musical_major_sharps() {
        assert_eq!(fixed("F#"), "02B");
        assert_eq!(fixed("C#"), "03B");
        assert_eq!(fixed("G#"), "04B");
        assert_eq!(fixed("D#"), "05B");
        assert_eq!(fixed("A#"), "06B");
    }

    #[test]
    fn fix_musical_minor_naturals() {
        assert_eq!(fixed("Am"), "08A");
        assert_eq!(fixed("Em"), "09A");
        assert_eq!(fixed("Bm"), "10A");
        assert_eq!(fixed("Fm"), "04A");
        assert_eq!(fixed("Cm"), "05A");
        assert_eq!(fixed("Gm"), "06A");
        assert_eq!(fixed("Dm"), "07A");
    }

    #[test]
    fn fix_musical_minor_flats() {
        assert_eq!(fixed("Gbm"), "11A");
        assert_eq!(fixed("Dbm"), "12A");
        assert_eq!(fixed("Abm"), "01A");
        assert_eq!(fixed("Ebm"), "02A");
        assert_eq!(fixed("Bbm"), "03A");
    }

    #[test]
    fn fix_musical_minor_sharps() {
        assert_eq!(fixed("F#m"), "11A");
        assert_eq!(fixed("C#m"), "12A");
        assert_eq!(fixed("G#m"), "01A");
        assert_eq!(fixed("D#m"), "02A");
        assert_eq!(fixed("A#m"), "03A");
    }
}
