use std::collections::HashMap;

use crate::keyboard::{
    keystroke_for_output_char, KEY_BACKSPACE, KEY_DELETE, KEY_DOWN, KEY_END, KEY_HOME, KEY_LEFT,
    KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_RIGHT, KEY_RIGHTSHIFT, KEY_UP,
};
use crate::model::{Action, KeyState};

#[derive(Debug, Default, Clone)]
struct EditorState {
    buf: Vec<char>,
    cursor: usize,
}

impl EditorState {
    fn insert_char(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) -> Option<char> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        Some(self.buf.remove(self.cursor))
    }

    fn delete(&mut self) -> Option<char> {
        if self.cursor >= self.buf.len() {
            return None;
        }
        Some(self.buf.remove(self.cursor))
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

    fn home(&mut self) {
        self.cursor = 0;
    }

    fn end(&mut self) {
        self.cursor = self.buf.len();
    }
}

#[derive(Debug, Default, Clone)]
struct CorrectionState {
    deleted_backspace: Vec<char>,
    deleted_delete: Vec<char>,
    inserted: String,
    started_at_end: bool,
}

impl CorrectionState {
    fn deleted_string(&self) -> String {
        self.deleted_backspace
            .iter()
            .rev()
            .chain(self.deleted_delete.iter())
            .collect()
    }

    fn has_replace(&self) -> bool {
        (!self.deleted_backspace.is_empty() || !self.deleted_delete.is_empty())
            && !self.inserted.is_empty()
    }
}

#[derive(Debug, Default, Clone)]
pub struct PlaybackTracer {
    keystrokes: HashMap<(u32, bool), char>,
    editor: EditorState,

    shift_down: bool,
    ctrl_down: bool,

    typing_run: String,
    correction: Option<CorrectionState>,

    pending_lines: Vec<String>,
}

impl PlaybackTracer {
    pub fn new() -> Self {
        Self {
            keystrokes: us_qwerty_keystroke_map(),
            ..Default::default()
        }
    }

    pub fn observe_action(&mut self, action: &Action) {
        let Action::Key { keycode, state } = action else {
            return;
        };
        match state {
            KeyState::Pressed => self.handle_key_pressed(*keycode),
            KeyState::Released => self.handle_key_released(*keycode),
        }
    }

    pub fn drain_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_lines)
    }

    pub fn finish(&mut self) -> Vec<String> {
        self.finish_correction();
        self.drain_lines()
    }

    fn decode_char(&self, keycode: u32) -> Option<char> {
        self.keystrokes.get(&(keycode, self.shift_down)).copied()
    }

    fn ensure_correction(&mut self) -> &mut CorrectionState {
        let started_at_end = self.editor.cursor == self.editor.buf.len();
        self.correction.get_or_insert_with(|| CorrectionState {
            started_at_end,
            ..Default::default()
        })
    }

    fn finish_correction(&mut self) {
        let Some(correction) = self.correction.take() else {
            return;
        };
        if !correction.has_replace() {
            return;
        }

        let wrong = correction.deleted_string();
        let correct = correction.inserted;
        self.pending_lines.push(format!(
            "Replace \"{}\" with \"{}\"...",
            escape_for_log(&wrong),
            escape_for_log(&correct)
        ));
    }

    fn flush_typing_run_on_edit(&mut self) {
        if self.typing_run.is_empty() {
            return;
        }
        let msg = format!("Typing \"{}\"...", escape_for_log(&self.typing_run));
        self.pending_lines.push(msg);
        self.typing_run.clear();
    }

    fn maybe_finish_correction_before_key(&mut self, keycode: u32, decoded_char: Option<char>) {
        let Some(correction) = &self.correction else {
            return;
        };
        if !correction.has_replace() {
            return;
        }

        if is_edit_key(keycode) {
            self.finish_correction();
            return;
        }

        if correction.started_at_end {
            if let Some(c) = decoded_char {
                if !is_word_char(c) {
                    self.finish_correction();
                }
            }
        }
    }

    fn handle_key_pressed(&mut self, keycode: u32) {
        if keycode == KEY_LEFTSHIFT || keycode == KEY_RIGHTSHIFT {
            self.shift_down = true;
            return;
        }
        if keycode == KEY_LEFTCTRL {
            self.ctrl_down = true;
            return;
        }

        let decoded_char = if self.ctrl_down {
            None
        } else {
            self.decode_char(keycode)
        };

        self.maybe_finish_correction_before_key(keycode, decoded_char);

        if is_edit_key(keycode) {
            self.flush_typing_run_on_edit();

            match keycode {
                KEY_LEFT => {
                    if self.ctrl_down {
                        self.editor.move_word_left();
                    } else {
                        self.editor.move_left();
                    }
                }
                KEY_RIGHT => {
                    if self.ctrl_down {
                        self.editor.move_word_right();
                    } else {
                        self.editor.move_right();
                    }
                }
                KEY_HOME => self.editor.home(),
                KEY_END => self.editor.end(),
                KEY_UP | KEY_DOWN => {}
                KEY_BACKSPACE => {
                    let deleted = self.editor.backspace();
                    if let Some(c) = deleted {
                        self.ensure_correction().deleted_backspace.push(c);
                    }
                }
                KEY_DELETE => {
                    let deleted = self.editor.delete();
                    if let Some(c) = deleted {
                        self.ensure_correction().deleted_delete.push(c);
                    }
                }
                _ => {}
            }

            return;
        }

        let Some(c) = decoded_char else {
            return;
        };

        self.editor.insert_char(c);

        if let Some(correction) = &mut self.correction {
            correction.inserted.push(c);
            return;
        }

        if self.editor.cursor == self.editor.buf.len() {
            self.typing_run.push(c);
        }
    }

    fn handle_key_released(&mut self, keycode: u32) {
        if keycode == KEY_LEFTSHIFT || keycode == KEY_RIGHTSHIFT {
            self.shift_down = false;
        }
        if keycode == KEY_LEFTCTRL {
            self.ctrl_down = false;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    pub action_index: usize,
    pub line: String,
}

/// Precompute console trace events so they can be printed *before* the associated
/// typing/correction sequence starts during playback.
pub fn plan_console_trace(actions: &[Action]) -> Vec<TraceEvent> {
    let mut planner = TracePlanner::new();
    for (action_index, action) in actions.iter().enumerate() {
        planner.observe_action(action_index, action);
    }
    planner.finish();

    planner.events.sort_by_key(|event| event.action_index);
    planner.events
}

#[derive(Debug, Default, Clone)]
struct ScheduledCorrection {
    start_action_index: usize,
    deleted_backspace: Vec<char>,
    deleted_delete: Vec<char>,
    inserted: String,
    left_end: bool,
}

impl ScheduledCorrection {
    fn deleted_string(&self) -> String {
        self.deleted_backspace
            .iter()
            .rev()
            .chain(self.deleted_delete.iter())
            .collect()
    }

    fn has_replace(&self) -> bool {
        (!self.deleted_backspace.is_empty() || !self.deleted_delete.is_empty())
            && !self.inserted.is_empty()
    }
}

#[derive(Debug, Default, Clone)]
struct TracePlanner {
    keystrokes: HashMap<(u32, bool), char>,
    editor: EditorState,

    shift_down: bool,
    ctrl_down: bool,

    typing_run_start_action: Option<usize>,
    typing_run: String,

    correction: Option<ScheduledCorrection>,
    events: Vec<TraceEvent>,
}

impl TracePlanner {
    fn new() -> Self {
        Self {
            keystrokes: us_qwerty_keystroke_map(),
            ..Default::default()
        }
    }

    fn observe_action(&mut self, action_index: usize, action: &Action) {
        let Action::Key { keycode, state } = action else {
            return;
        };
        match state {
            KeyState::Pressed => self.handle_key_pressed(action_index, *keycode),
            KeyState::Released => self.handle_key_released(*keycode),
        }
    }

    fn finish(&mut self) {
        self.finish_correction();
    }

    fn decode_char(&self, keycode: u32) -> Option<char> {
        self.keystrokes.get(&(keycode, self.shift_down)).copied()
    }

    fn finish_correction(&mut self) {
        let Some(correction) = self.correction.take() else {
            return;
        };
        if !correction.has_replace() {
            return;
        }

        let start_action_index = correction.start_action_index;
        let wrong = correction.deleted_string();
        let correct = correction.inserted;
        self.events.push(TraceEvent {
            action_index: start_action_index,
            line: format!(
                "Replace \"{}\" with \"{}\"...",
                escape_for_log(&wrong),
                escape_for_log(&correct)
            ),
        });
    }

    fn maybe_finish_correction_before_key(&mut self, keycode: u32, decoded_char: Option<char>) {
        let Some(correction) = &self.correction else {
            return;
        };

        let at_end = self.editor.cursor == self.editor.buf.len();

        let should_finish = if correction.left_end {
            at_end && (is_edit_key(keycode) || decoded_char.is_some())
        } else if correction.has_replace() {
            if is_edit_key(keycode) {
                true
            } else {
                match decoded_char {
                    Some(c) => !is_word_char(c),
                    None => false,
                }
            }
        } else {
            false
        };

        if should_finish {
            self.finish_correction();
        }
    }

    fn flush_typing_run_on_edit(&mut self) {
        let Some(start_idx) = self.typing_run_start_action else {
            self.typing_run.clear();
            return;
        };
        if self.typing_run.is_empty() {
            self.typing_run_start_action = None;
            return;
        }

        self.events.push(TraceEvent {
            action_index: start_idx,
            line: format!("Typing \"{}\"...", escape_for_log(&self.typing_run)),
        });
        self.typing_run.clear();
        self.typing_run_start_action = None;
    }

    fn handle_key_pressed(&mut self, action_index: usize, keycode: u32) {
        if keycode == KEY_LEFTSHIFT || keycode == KEY_RIGHTSHIFT {
            self.shift_down = true;
            return;
        }
        if keycode == KEY_LEFTCTRL {
            self.ctrl_down = true;
            return;
        }

        let decoded_char = if self.ctrl_down {
            None
        } else {
            self.decode_char(keycode)
        };

        self.maybe_finish_correction_before_key(keycode, decoded_char);

        if is_edit_key(keycode) {
            self.flush_typing_run_on_edit();

            if self.correction.is_none() {
                self.correction = Some(ScheduledCorrection {
                    start_action_index: action_index,
                    left_end: self.editor.cursor < self.editor.buf.len(),
                    ..Default::default()
                });
            }

            match keycode {
                KEY_LEFT => {
                    if self.ctrl_down {
                        self.editor.move_word_left();
                    } else {
                        self.editor.move_left();
                    }
                }
                KEY_RIGHT => {
                    if self.ctrl_down {
                        self.editor.move_word_right();
                    } else {
                        self.editor.move_right();
                    }
                }
                KEY_HOME => self.editor.home(),
                KEY_END => self.editor.end(),
                KEY_UP | KEY_DOWN => {}
                KEY_BACKSPACE => {
                    let deleted = self.editor.backspace();
                    if let Some(c) = deleted {
                        if let Some(correction) = &mut self.correction {
                            correction.deleted_backspace.push(c);
                        }
                    }
                }
                KEY_DELETE => {
                    let deleted = self.editor.delete();
                    if let Some(c) = deleted {
                        if let Some(correction) = &mut self.correction {
                            correction.deleted_delete.push(c);
                        }
                    }
                }
                _ => {}
            }

            if let Some(correction) = &mut self.correction {
                correction.left_end |= self.editor.cursor < self.editor.buf.len();
            }

            return;
        }

        let Some(c) = decoded_char else {
            return;
        };

        self.editor.insert_char(c);

        if let Some(correction) = &mut self.correction {
            correction.inserted.push(c);
            correction.left_end |= self.editor.cursor < self.editor.buf.len();
            return;
        }

        if self.editor.cursor == self.editor.buf.len() {
            if self.typing_run.is_empty() {
                self.typing_run_start_action = Some(action_index);
            }
            self.typing_run.push(c);
        }
    }

    fn handle_key_released(&mut self, keycode: u32) {
        if keycode == KEY_LEFTSHIFT || keycode == KEY_RIGHTSHIFT {
            self.shift_down = false;
        }
        if keycode == KEY_LEFTCTRL {
            self.ctrl_down = false;
        }
    }
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

fn is_edit_key(keycode: u32) -> bool {
    matches!(
        keycode,
        KEY_LEFT | KEY_RIGHT | KEY_UP | KEY_DOWN | KEY_HOME | KEY_END | KEY_BACKSPACE | KEY_DELETE
    )
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '\''
}

fn escape_for_log(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}
