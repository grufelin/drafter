use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::keyboard::{
    keystroke_for_output_char, KEY_BACKSPACE, KEY_DELETE, KEY_LEFT, KEY_LEFTCTRL, KEY_LEFTSHIFT,
    KEY_RIGHT, KEY_RIGHTSHIFT,
};
use crate::model::{Action, KeyState, Plan};

#[derive(Debug, Clone, Copy, Default)]
pub struct PlanStats {
    pub actions: usize,
    pub key_events: usize,
    pub modifier_updates: usize,
    pub total_wait_ms: u64,
}

pub fn stats(plan: &Plan) -> PlanStats {
    let mut out = PlanStats {
        actions: plan.actions.len(),
        ..Default::default()
    };

    for a in &plan.actions {
        match a {
            Action::Wait { ms } => {
                out.total_wait_ms = out.total_wait_ms.saturating_add(*ms);
            }
            Action::Modifiers { .. } => out.modifier_updates += 1,
            Action::Key { .. } => out.key_events += 1,
        }
    }

    out
}

#[derive(Debug, Default, Clone)]
struct SimEditorState {
    buf: Vec<char>,
    cursor: usize,
}

impl SimEditorState {
    fn insert_char(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        self.buf.remove(self.cursor);
    }

    fn delete(&mut self) {
        if self.cursor >= self.buf.len() {
            return;
        }
        self.buf.remove(self.cursor);
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.buf.len() {
            self.cursor += 1;
        }
    }

    fn move_word_left(&mut self) {
        self.cursor = crate::word_nav::ctrl_left(&self.buf, self.cursor, is_word_char);
    }

    fn move_word_right(&mut self) {
        self.cursor = crate::word_nav::ctrl_right(&self.buf, self.cursor, is_word_char);
    }

    fn as_string(&self) -> String {
        self.buf.iter().collect()
    }
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '\''
}

fn us_qwerty_keystroke_map() -> HashMap<(u32, bool), char> {
    let mut map = HashMap::new();

    let mut candidates = Vec::new();
    candidates.push('\n');
    candidates.push(' ');
    for b in 33u8..=126u8 {
        candidates.push(b as char);
    }

    for c in candidates {
        if let Some(stroke) = keystroke_for_output_char(c) {
            map.insert((stroke.keycode, stroke.shift), c);
        }
    }

    map
}

/// Simulate the final editor text produced by a plan.
///
/// This is intended for tests/debugging. It applies basic cursor movement and
/// insertion/deletion for a US-QWERTY layout. It does not model editor-specific
/// behaviors such as smart-quote auto-substitution.
pub fn simulate_typed_text(plan: &Plan) -> Result<String> {
    let mut editor = SimEditorState::default();
    let mut shift_down = false;
    let mut ctrl_down = false;
    let keystrokes = us_qwerty_keystroke_map();

    for action in &plan.actions {
        let Action::Key { keycode, state } = action else {
            continue;
        };

        match (*keycode, *state) {
            (KEY_LEFTSHIFT | KEY_RIGHTSHIFT, KeyState::Pressed) => {
                shift_down = true;
                continue;
            }
            (KEY_LEFTSHIFT | KEY_RIGHTSHIFT, KeyState::Released) => {
                shift_down = false;
                continue;
            }
            (KEY_LEFTCTRL, KeyState::Pressed) => {
                ctrl_down = true;
                continue;
            }
            (KEY_LEFTCTRL, KeyState::Released) => {
                ctrl_down = false;
                continue;
            }
            (_, KeyState::Released) => continue,
            _ => {}
        }

        match *keycode {
            KEY_LEFT => {
                if ctrl_down {
                    editor.move_word_left();
                } else {
                    editor.move_left();
                }
            }
            KEY_RIGHT => {
                if ctrl_down {
                    editor.move_word_right();
                } else {
                    editor.move_right();
                }
            }
            KEY_BACKSPACE => editor.backspace(),
            KEY_DELETE => editor.delete(),
            _ => {
                if ctrl_down {
                    return Err(anyhow!(
                        "simulate_typed_text does not support Ctrl+keycode {keycode}"
                    ));
                }

                let c = keystrokes
                    .get(&(*keycode, shift_down))
                    .copied()
                    .ok_or_else(|| {
                        anyhow!(
                            "simulate_typed_text does not support keycode {keycode} (shift={shift_down})"
                        )
                    })?;

                editor.insert_char(c);
            }
        }
    }

    Ok(editor.as_string())
}
