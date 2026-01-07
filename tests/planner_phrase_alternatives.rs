use rand::rngs::StdRng;
use rand::SeedableRng;

use drafter::keyboard::{KEY_BACKSPACE, KEY_LEFT, KEY_Z};
use drafter::llm::PhraseAlternative;
use drafter::model::Action;
use drafter::planner::{generate_plan_with_phrase_alternatives, PlannerConfig};
use drafter::sim::simulate_typed_text;

#[test]
fn generates_plan_with_llm_phrase_alternative_edits() {
    let final_text = "HelloWorld";

    let cfg = PlannerConfig {
        wpm_min: 55.0,
        wpm_max: 55.0,
        error_rate_per_word: 0.0,
        immediate_fix_rate: 0.0,
        ..Default::default()
    };

    let alternatives_by_paragraph = vec![vec![PhraseAlternative {
        original: "Hello".to_string(),
        alternative: "zzz".to_string(),
    }]];

    let mut rng = StdRng::seed_from_u64(7);
    let plan = generate_plan_with_phrase_alternatives(
        final_text,
        cfg,
        &alternatives_by_paragraph,
        &mut rng,
    )
    .expect("plan generation should succeed");

    let mut saw_z = false;
    let mut saw_left = false;
    let mut saw_backspace = false;

    for action in &plan.actions {
        if let Action::Key { keycode, .. } = action {
            if *keycode == KEY_Z {
                saw_z = true;
            }
            if *keycode == KEY_LEFT {
                saw_left = true;
            }
            if *keycode == KEY_BACKSPACE {
                saw_backspace = true;
            }
        }
    }

    let simulated = simulate_typed_text(&plan).expect("plan simulation should succeed");
    assert_eq!(simulated, final_text);

    assert!(saw_z, "expected typing to include the alternative phrase");
    assert!(
        saw_left,
        "expected at least one cursor move left for corrections"
    );
    assert!(
        saw_backspace,
        "expected at least one backspace for corrections"
    );
}
