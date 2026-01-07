use drafter::keyboard::{
    keystroke_for_output_char, KEY_BACKSPACE, KEY_LEFT, KEY_LEFTCTRL, KEY_RIGHT,
};
use drafter::model::{Action, KeyState, Plan, PlanConfig};
use drafter::sim::simulate_typed_text;

fn key_presses_for_text(text: &str) -> Vec<Action> {
    text.chars()
        .map(|c| {
            let stroke = keystroke_for_output_char(c).expect("test text must be typable");
            Action::Key {
                keycode: stroke.keycode,
                state: KeyState::Pressed,
            }
        })
        .collect()
}

fn dummy_plan(actions: Vec<Action>) -> Plan {
    Plan {
        version: 1,
        config: PlanConfig {
            layout: "us".to_string(),
            keymap_format: 1,
            keymap: String::new(),
            wpm_target: 0.0,
        },
        actions,
    }
}

#[test]
fn simulate_supports_ctrl_left_word_nav() {
    let mut actions = key_presses_for_text("hello world");

    actions.push(Action::Key {
        keycode: KEY_LEFTCTRL,
        state: KeyState::Pressed,
    });
    actions.push(Action::Key {
        keycode: KEY_LEFT,
        state: KeyState::Pressed,
    });
    actions.push(Action::Key {
        keycode: KEY_LEFTCTRL,
        state: KeyState::Released,
    });

    actions.push(Action::Key {
        keycode: KEY_BACKSPACE,
        state: KeyState::Pressed,
    });

    let plan = dummy_plan(actions);
    let out = simulate_typed_text(&plan).expect("plan simulation should succeed");
    assert_eq!(out, "helloworld");
}

#[test]
fn simulate_supports_ctrl_right_word_nav() {
    let mut actions = key_presses_for_text("hello world");

    actions.push(Action::Key {
        keycode: KEY_LEFTCTRL,
        state: KeyState::Pressed,
    });
    actions.push(Action::Key {
        keycode: KEY_LEFT,
        state: KeyState::Pressed,
    });
    actions.push(Action::Key {
        keycode: KEY_RIGHT,
        state: KeyState::Pressed,
    });
    actions.push(Action::Key {
        keycode: KEY_LEFTCTRL,
        state: KeyState::Released,
    });

    actions.push(Action::Key {
        keycode: KEY_BACKSPACE,
        state: KeyState::Pressed,
    });

    let plan = dummy_plan(actions);
    let out = simulate_typed_text(&plan).expect("plan simulation should succeed");
    assert_eq!(out, "hello worl");
}
