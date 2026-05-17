use std::path::PathBuf;

use crate::{
    fields::CountField,
    linter::{CheckOutcome, LintTarget, Rule, RuleMetadata},
    rule_metadata,
    track::{Track, TrackFields, TrackFile},
};

pub fn make_track() -> Track {
    Track {
        file: TrackFile {
            file_path: PathBuf::from("Publisher/[RLS] Album/Disc 2/01. [10A] Artist - Title.mp3"),
            mtime: 1,
        },
        fields: TrackFields {
            media_hash: Some("abc123".to_string()),
            artist: Some("Artist".to_string()),
            title: Some("Title".to_string()),
            album: Some("Album".to_string()),
            remixer: Some("Remixer".to_string()),
            publisher: Some("Publisher".to_string()),
            catalog_id: Some("RLS".to_string()),
            year: Some("2015".to_string()),
            genre: Some("Genre".to_string()),
            key: Some("10A".to_string()),
            bpm: Some("170".to_string()),
            disc: Some("2/4".parse::<CountField>().unwrap()),
            track: Some("1/10".parse::<CountField>().unwrap()),
        },
    }
}

// Minimal rules that exercise distinct engine code paths. Use these
// instead of real rules in tests so the assertions don't depend on
// production rule behavior (which evolves) and so each test can pick
// the exact outcome it needs.

/// Always returns [`CheckOutcome::Passed`]. Useful as filler when a
/// test needs the engine to have *some* rule installed but wants its
/// output to be uninteresting.
static PASS_META: RuleMetadata = rule_metadata! {
    id: "test.always-pass",
    description: "Always passes.",
};
pub struct AlwaysPass;
impl Rule for AlwaysPass {
    fn metadata(&self) -> &'static RuleMetadata {
        &PASS_META
    }
    fn check(&self, _: &LintTarget) -> CheckOutcome {
        CheckOutcome::Passed
    }
}

/// Always produces an error-severity violation with no attached fix.
/// Drives "the engine should leave unfixable violations in the final
/// results" cases and "the autofix loop should still terminate when a
/// rule keeps failing but offers no fix" cases.
static FAIL_NO_FIX_META: RuleMetadata = rule_metadata! {
    id: "test.always-fail-no-fix",
    description: "Always fails with no fix.",
};
pub struct AlwaysFailNoFix;
impl Rule for AlwaysFailNoFix {
    fn metadata(&self) -> &'static RuleMetadata {
        &FAIL_NO_FIX_META
    }
    fn check(&self, _: &LintTarget) -> CheckOutcome {
        self.error("nope").into()
    }
}

/// Always returns [`CheckOutcome::Skipped`]. Drives tests that assert
/// the engine preserves skip outcomes through the autofix loop rather
/// than collapsing them to `Passed`.
static SKIP_META: RuleMetadata = rule_metadata! {
    id: "test.always-skip",
    description: "Always skips.",
};
pub struct AlwaysSkip;
impl Rule for AlwaysSkip {
    fn metadata(&self) -> &'static RuleMetadata {
        &SKIP_META
    }
    fn check(&self, _: &LintTarget) -> CheckOutcome {
        CheckOutcome::skipped("no inputs")
    }
}

// Convergence chain: title progresses "start" -> "middle" -> "end".
// [`StepA`] fires only on "start"; [`StepB`] only on "middle". Neither
// re-triggers the other, so the autofix loop converges in two
// iterations.

/// First link in the convergence chain: rewrites title `"start"` ->
/// `"middle"`. No-op for any other title. Pair with [`StepB`] to
/// exercise multi-iteration convergence without oscillation.
static STEP_A_META: RuleMetadata = rule_metadata! {
    id: "test.step-a",
    description: "Sets title 'start' to 'middle'.",
};
pub struct StepA;
impl Rule for StepA {
    fn metadata(&self) -> &'static RuleMetadata {
        &STEP_A_META
    }
    fn check(&self, target: &LintTarget) -> CheckOutcome {
        if target.track.fields.title.as_deref() == Some("start") {
            self.error("start -> middle")
                .with_fix(|t| t.fields.title = Some("middle".to_string()))
                .into()
        } else {
            CheckOutcome::Passed
        }
    }
}

/// Second link in the convergence chain: rewrites title `"middle"` ->
/// `"end"`. No-op for any other title. See [`StepA`].
static STEP_B_META: RuleMetadata = rule_metadata! {
    id: "test.step-b",
    description: "Sets title 'middle' to 'end'.",
};
pub struct StepB;
impl Rule for StepB {
    fn metadata(&self) -> &'static RuleMetadata {
        &STEP_B_META
    }
    fn check(&self, target: &LintTarget) -> CheckOutcome {
        if target.track.fields.title.as_deref() == Some("middle") {
            self.error("middle -> end")
                .with_fix(|t| t.fields.title = Some("end".to_string()))
                .into()
        } else {
            CheckOutcome::Passed
        }
    }
}

// Oscillating pair: title flips between "a" and "b" forever. Used to
// verify the engine's iteration cap actually bounds the loop and
// reports `hit_max_iterations` when it does.

/// Oscillation half-cycle: rewrites title `"b"` -> `"a"`. Pair with
/// [`OscToB`] to construct an autofix loop that never converges.
static OSC_TO_A_META: RuleMetadata = rule_metadata! {
    id: "test.osc-to-a",
    description: "If title is 'b', set it to 'a'.",
};
pub struct OscToA;
impl Rule for OscToA {
    fn metadata(&self) -> &'static RuleMetadata {
        &OSC_TO_A_META
    }
    fn check(&self, target: &LintTarget) -> CheckOutcome {
        if target.track.fields.title.as_deref() == Some("b") {
            self.error("b -> a")
                .with_fix(|t| t.fields.title = Some("a".to_string()))
                .into()
        } else {
            CheckOutcome::Passed
        }
    }
}

/// Oscillation half-cycle: rewrites title `"a"` -> `"b"`. See
/// [`OscToA`].
static OSC_TO_B_META: RuleMetadata = rule_metadata! {
    id: "test.osc-to-b",
    description: "If title is 'a', set it to 'b'.",
};
pub struct OscToB;
impl Rule for OscToB {
    fn metadata(&self) -> &'static RuleMetadata {
        &OSC_TO_B_META
    }
    fn check(&self, target: &LintTarget) -> CheckOutcome {
        if target.track.fields.title.as_deref() == Some("a") {
            self.error("a -> b")
                .with_fix(|t| t.fields.title = Some("b".to_string()))
                .into()
        } else {
            CheckOutcome::Passed
        }
    }
}
