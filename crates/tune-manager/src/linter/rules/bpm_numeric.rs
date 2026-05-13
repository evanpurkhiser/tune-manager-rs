use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{LintResult, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "bpm.numeric",
    description: r#"
        BPM must be numeric with strict decimal formatting.

        Valid:
        - 170
        - 128.5
        - 128.25

        Invalid:
        - fast (not numeric)
        - 12x (contains non-numeric characters)
        - 128.345 (more than two decimal places)
        - 170.50 (trailing zero in decimal form)
        - 170.00 (whole numbers should not be decimal)
    "#,
    autofix_notes: r#"
        Emits one violation per detected issue. Each fix addresses just
        that issue:
        - More than two decimal places → rounds to two.
        - Trailing decimal zero → strips trailing zeros (and the dot if
          nothing remains after it).

        Non-numeric values (`fast`, `12x`) are not fixed — the intended
        value can't be recovered mechanically.
    "#,
};

static NUMERIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]+(\.[0-9]+)?$").unwrap());

pub struct BpmNumericRule;

impl Rule for BpmNumericRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        let Some(bpm) = track.tags.bpm.as_deref() else {
            return LintResult::Passed;
        };
        let trimmed = bpm.trim();

        if !NUMERIC_RE.is_match(trimmed) {
            return self
                .error(format!("BPM is not numeric: `{trimmed}`"))
                .into();
        }

        let mut violations = vec![];
        let decimals = trimmed.split_once('.').map(|(_, d)| d).unwrap_or("");

        if decimals.len() > 2 {
            violations.push(
                self.error("BPM has more than 2 decimal places")
                    .with_fix(fix_round_to_two_decimals),
            );
        }

        if !decimals.is_empty() && decimals.ends_with('0') {
            violations.push(
                self.error("BPM has trailing decimal zero")
                    .with_fix(fix_strip_trailing_decimal_zero),
            );
        }

        violations.into()
    }
}

fn fix_round_to_two_decimals(track: &mut Track) {
    let Some(bpm) = track.tags.bpm.as_deref() else {
        return;
    };
    let Ok(n) = bpm.trim().parse::<f64>() else {
        return;
    };
    track.tags.bpm = Some(format!("{:.2}", n));
}

fn fix_strip_trailing_decimal_zero(track: &mut Track) {
    let Some(bpm) = track.tags.bpm.as_deref() else {
        return;
    };
    let Some((int, dec)) = bpm.trim().split_once('.') else {
        return;
    };
    let stripped = dec.trim_end_matches('0');
    let result = if stripped.is_empty() {
        int.to_string()
    } else {
        format!("{int}.{stripped}")
    };
    track.tags.bpm = Some(result);
}

#[cfg(test)]
mod tests {
    use super::BpmNumericRule;
    use crate::linter::{Rule, test_utils::make_track};

    fn check_with(bpm: &str) -> crate::linter::LintResult {
        let mut track = make_track();
        track.tags.bpm = Some(bpm.to_string());
        BpmNumericRule.check(&track)
    }

    fn fix_all(bpm: &str) -> String {
        let mut track = make_track();
        track.tags.bpm = Some(bpm.to_string());
        let result = BpmNumericRule.check(&track);
        for v in result.violations() {
            if let Some(fix) = &v.fix {
                fix.apply(&mut track);
            }
        }
        track.tags.bpm.unwrap()
    }

    #[test]
    fn ok_integer() {
        assert!(check_with("170").is_passed());
    }

    #[test]
    fn ok_one_decimal() {
        assert!(check_with("128.5").is_passed());
    }

    #[test]
    fn ok_two_decimals() {
        assert!(check_with("128.25").is_passed());
    }

    #[test]
    fn whitespace_alone_is_not_a_violation() {
        // Whitespace is handled by meta.text-trimmed (planned), not here.
        assert!(check_with(" 128.5 ").is_passed());
    }

    #[test]
    fn fail_non_numeric() {
        assert_eq!(check_with("fast").violations().len(), 1);
    }

    #[test]
    fn fail_partial_alpha() {
        assert_eq!(check_with("12x").violations().len(), 1);
    }

    #[test]
    fn fail_too_many_decimals() {
        assert_eq!(check_with("128.345").violations().len(), 1);
    }

    #[test]
    fn fail_trailing_zero() {
        assert_eq!(check_with("170.50").violations().len(), 1);
    }

    #[test]
    fn fail_double_zero_decimal() {
        assert_eq!(check_with("170.00").violations().len(), 1);
    }

    #[test]
    fn two_violations_for_too_many_and_trailing_zero() {
        // 170.500 has both: 3 decimals (too many) AND trailing zero.
        let result = check_with("170.500");
        assert_eq!(result.violations().len(), 2);
    }

    #[test]
    fn non_numeric_is_unfixable() {
        let result = check_with("fast");
        assert!(result.violations()[0].fix.is_none());
    }

    #[test]
    fn fix_too_many_decimals_rounds_to_two() {
        assert_eq!(fix_all("128.123"), "128.12");
    }

    #[test]
    fn fix_trailing_zero_strips() {
        assert_eq!(fix_all("170.50"), "170.5");
    }

    #[test]
    fn fix_double_zero_drops_decimal() {
        assert_eq!(fix_all("170.00"), "170");
    }

    #[test]
    fn fix_combined_too_many_and_trailing_zero() {
        // round to 2 → 170.50, then strip trailing zero → 170.5
        assert_eq!(fix_all("170.500"), "170.5");
    }

    #[test]
    fn fix_combined_to_integer() {
        // round to 2 → 170.00, then strip trailing zero → 170
        assert_eq!(fix_all("170.000"), "170");
    }
}
