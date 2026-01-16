use rand::rngs::StdRng;
use rand::SeedableRng;

use drafter::keyboard::{KEY_APOSTROPHE, KEY_BACKSPACE, KEY_LEFT, KEY_RIGHT};
use drafter::model::Action;
use drafter::planner::{generate_plan, PlannerConfig};

#[test]
fn generates_plan_with_edits_and_review_pass() {
    let final_text = "Hello world.\n\nThis is a test paragraph with several words, and it should include a couple of errors.\nAnother sentence ends here.\n";

    let cfg = PlannerConfig {
        wpm_min: 55.0,
        wpm_max: 55.0,
        error_rate_per_word: 0.45,
        immediate_fix_rate: 0.0,
        ..Default::default()
    };

    let mut rng = StdRng::seed_from_u64(123);
    let plan = generate_plan(final_text, cfg, &mut rng).expect("plan generation should succeed");

    assert_eq!(plan.config.layout, "us");
    assert!(!plan.actions.is_empty());

    let mut saw_left = false;
    let mut saw_backspace = false;

    for a in &plan.actions {
        if let Action::Key { keycode, .. } = a {
            if *keycode == KEY_LEFT {
                saw_left = true;
            }
            if *keycode == KEY_BACKSPACE {
                saw_backspace = true;
            }
        }
    }

    assert!(
        saw_left,
        "expected at least one cursor move left for corrections"
    );
    assert!(
        saw_backspace,
        "expected at least one backspace for corrections"
    );
}

#[test]
fn supports_smart_apostrophe_in_final_draft() {
    let final_text = "The casino’s catalogue is updated.\n";

    let cfg = PlannerConfig {
        wpm_min: 55.0,
        wpm_max: 55.0,
        error_rate_per_word: 0.0,
        immediate_fix_rate: 0.0,
        ..Default::default()
    };

    let mut rng = StdRng::seed_from_u64(42);
    let plan = generate_plan(final_text, cfg, &mut rng).expect("plan generation should succeed");

    let mut saw_apostrophe_key = false;
    for a in &plan.actions {
        if let Action::Key { keycode, .. } = a {
            if *keycode == KEY_APOSTROPHE {
                saw_apostrophe_key = true;
                break;
            }
        }
    }

    assert!(
        saw_apostrophe_key,
        "expected an apostrophe key event for smart apostrophe output"
    );
}

#[test]
fn error_rate_zero_is_straight_through() {
    let final_text = "Hello world. This should type cleanly.\n";

    let cfg = PlannerConfig {
        wpm_min: 55.0,
        wpm_max: 55.0,
        error_rate_per_word: 0.0,
        immediate_fix_rate: 1.0,
        review_pause_ms_min: 99_999,
        review_pause_ms_max: 99_999,
        ..Default::default()
    };

    let mut rng = StdRng::seed_from_u64(0);
    let plan = generate_plan(final_text, cfg, &mut rng).expect("plan generation should succeed");

    for action in &plan.actions {
        match action {
            Action::Key { keycode, .. } => {
                assert_ne!(
                    *keycode, KEY_BACKSPACE,
                    "expected no backspaces in no-revision mode"
                );
                assert_ne!(
                    *keycode, KEY_LEFT,
                    "expected no cursor left moves in no-revision mode"
                );
                assert_ne!(
                    *keycode, KEY_RIGHT,
                    "expected no cursor right moves in no-revision mode"
                );
            }
            Action::Wait { ms } => {
                assert_ne!(
                    *ms, 99_999,
                    "expected no near-end review pause in no-revision mode"
                );
            }
            _ => {}
        }
    }
}
