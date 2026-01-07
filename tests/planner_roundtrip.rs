use rand::rngs::StdRng;
use rand::SeedableRng;

use drafter::keyboard::{KEY_APOSTROPHE, KEY_BACKSPACE, KEY_LEFT};
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
