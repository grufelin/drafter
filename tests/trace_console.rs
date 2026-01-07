use drafter::keyboard::{keystroke_for_output_char, KEY_BACKSPACE, KEY_LEFT, KEY_RIGHT};
use drafter::model::{Action, KeyState};
use drafter::trace::plan_console_trace;

fn actions_for_text(text: &str) -> Vec<Action> {
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

fn trace_events(actions: &[Action]) -> Vec<(usize, String)> {
    plan_console_trace(actions)
        .into_iter()
        .map(|e| (e.action_index, e.line))
        .collect()
}

#[test]
fn logs_typing_run_before_it_starts() {
    let mut actions = actions_for_text("a\nb");
    actions.push(Action::Key {
        keycode: KEY_LEFT,
        state: KeyState::Pressed,
    });

    let events = trace_events(&actions);

    assert_eq!(events, vec![(0, "Typing \"a\\nb\"...".to_string())]);
}

#[test]
fn logs_replace_before_the_correction_sequence() {
    let mut actions = actions_for_text("hello wurld.");
    let typing_len = actions.len();

    actions.push(Action::Key {
        keycode: KEY_LEFT,
        state: KeyState::Pressed,
    });

    for _ in 0..5 {
        actions.push(Action::Key {
            keycode: KEY_BACKSPACE,
            state: KeyState::Pressed,
        });
    }

    actions.extend(actions_for_text("world"));

    actions.push(Action::Key {
        keycode: KEY_RIGHT,
        state: KeyState::Pressed,
    });

    let events = trace_events(&actions);

    assert_eq!(
        events,
        vec![
            (0, "Typing \"hello wurld.\"...".to_string()),
            (
                typing_len,
                "Replace \"wurld\" with \"world\"...".to_string()
            ),
        ]
    );
}

#[test]
fn does_not_log_typing_run_at_end_of_plan() {
    let actions = actions_for_text("abc");
    let events = trace_events(&actions);
    assert!(events.is_empty());
}
