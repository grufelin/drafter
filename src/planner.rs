use anyhow::{anyhow, ensure, Result};
use rand::Rng;
use rand_distr::{Distribution, Normal};

use crate::keyboard::{
    find_first_unsupported_char, keystroke_for_output_char, qwerty_adjacent_char, KeyStroke,
    KEY_BACKSPACE, KEY_LEFT, KEY_RIGHT,
};
use crate::keymap::us_qwerty_keymap;
use crate::llm::{validate_phrase_alternatives, PhraseAlternative};
use crate::model::{Action, KeyState, Plan, PlanConfig};
use crate::word_nav_profile::{compatible_ctrl_jump_is_safe, WordNavProfile};

#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub wpm_min: f64,
    pub wpm_max: f64,
    pub error_rate_per_word: f64,
    pub word_variant_share: f64,
    pub immediate_fix_rate: f64,
    pub word_nav_profile: WordNavProfile,
    pub max_outstanding_errors: usize,
    pub stop_corrections_after_progress: f64,
    pub review_pause_ms_min: u64,
    pub review_pause_ms_max: u64,
    pub no_revision: bool,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            wpm_min: 40.0,
            wpm_max: 60.0,
            error_rate_per_word: 0.05,
            word_variant_share: 0.35,
            immediate_fix_rate: 0.35,
            word_nav_profile: WordNavProfile::Chrome,
            max_outstanding_errors: 4,
            stop_corrections_after_progress: 0.88,
            review_pause_ms_min: 1200,
            review_pause_ms_max: 2600,
            no_revision: false,
        }
    }
}

fn validate_config(cfg: &PlannerConfig) -> Result<()> {
    ensure!(cfg.wpm_min.is_finite(), "wpm_min must be finite");
    ensure!(cfg.wpm_max.is_finite(), "wpm_max must be finite");
    ensure!(
        cfg.wpm_min > 0.0 && cfg.wpm_max > 0.0,
        "wpm_min and wpm_max must be > 0"
    );
    ensure!(cfg.wpm_min <= cfg.wpm_max, "wpm_min must be <= wpm_max");

    ensure!(
        (0.0..=1.0).contains(&cfg.error_rate_per_word),
        "error_rate_per_word must be between 0.0 and 1.0"
    );
    ensure!(
        (0.0..=1.0).contains(&cfg.word_variant_share),
        "word_variant_share must be between 0.0 and 1.0"
    );
    ensure!(
        (0.0..=1.0).contains(&cfg.immediate_fix_rate),
        "immediate_fix_rate must be between 0.0 and 1.0"
    );
    ensure!(
        (0.0..=1.0).contains(&cfg.stop_corrections_after_progress),
        "stop_corrections_after_progress must be between 0.0 and 1.0"
    );

    ensure!(
        cfg.review_pause_ms_min <= cfg.review_pause_ms_max,
        "review_pause_ms_min must be <= review_pause_ms_max"
    );

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorrectionConstraint {
    None,
    SentenceOrParagraphBoundary,
}

#[derive(Debug, Clone)]
struct OutstandingError {
    start: usize,
    wrong: String,
    correct: String,
    fix_after_chars: usize,
    constraint: CorrectionConstraint,
}

#[derive(Debug, Default, Clone)]
struct EditorState {
    buf: Vec<char>,
    cursor: usize,
}

impl EditorState {
    fn insert_char(&mut self, c: char) {
        assert!(self.cursor <= self.buf.len());
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

#[derive(Debug, Clone)]
struct ActionBuilder {
    actions: Vec<Action>,
    shift_down: bool,
    ctrl_down: bool,
    shift_mask: u32,
    ctrl_mask: u32,
}

impl ActionBuilder {
    fn new(shift_mask: u32, ctrl_mask: u32) -> Self {
        Self {
            actions: Vec::new(),
            shift_down: false,
            ctrl_down: false,
            shift_mask,
            ctrl_mask,
        }
    }

    fn into_actions(self) -> Vec<Action> {
        self.actions
    }

    fn wait(&mut self, ms: u64) {
        if ms == 0 {
            return;
        }
        self.actions.push(Action::Wait { ms });
    }

    fn key(&mut self, keycode: u32, state: KeyState) {
        self.actions.push(Action::Key { keycode, state });
    }

    fn set_modifiers(&mut self) {
        let mut depressed = 0u32;
        if self.shift_down {
            depressed |= self.shift_mask;
        }
        if self.ctrl_down {
            depressed |= self.ctrl_mask;
        }

        self.actions.push(Action::Modifiers {
            mods_depressed: depressed,
            mods_latched: 0,
            mods_locked: 0,
            group: 0,
        });
    }

    fn set_shift(&mut self, down: bool, rng: &mut impl Rng) {
        if self.shift_down == down {
            return;
        }
        // Use left shift for simplicity.
        const KEY_LEFTSHIFT: u32 = crate::keyboard::KEY_LEFTSHIFT;

        if down {
            self.key(KEY_LEFTSHIFT, KeyState::Pressed);
            self.wait(rng.gen_range(5..=20));
            self.shift_down = true;
            self.set_modifiers();
            self.wait(rng.gen_range(0..=12));
        } else {
            self.key(KEY_LEFTSHIFT, KeyState::Released);
            self.wait(rng.gen_range(5..=20));
            self.shift_down = false;
            self.set_modifiers();
            self.wait(rng.gen_range(0..=12));
        }
    }

    fn set_ctrl(&mut self, down: bool, rng: &mut impl Rng) {
        if self.ctrl_down == down {
            return;
        }
        const KEY_LEFTCTRL: u32 = crate::keyboard::KEY_LEFTCTRL;

        if down {
            self.key(KEY_LEFTCTRL, KeyState::Pressed);
            self.wait(rng.gen_range(5..=20));
            self.ctrl_down = true;
            self.set_modifiers();
            self.wait(rng.gen_range(0..=12));
        } else {
            self.key(KEY_LEFTCTRL, KeyState::Released);
            self.wait(rng.gen_range(5..=20));
            self.ctrl_down = false;
            self.set_modifiers();
            self.wait(rng.gen_range(0..=12));
        }
    }

    fn press_key(&mut self, keycode: u32, rng: &mut impl Rng) {
        let hold_ms = rng.gen_range(18..=70);
        self.key(keycode, KeyState::Pressed);
        self.wait(hold_ms);
        self.key(keycode, KeyState::Released);
    }

    fn type_char(&mut self, stroke: KeyStroke, rng: &mut impl Rng) {
        self.set_ctrl(false, rng);
        self.set_shift(stroke.shift, rng);
        self.press_key(stroke.keycode, rng);
    }

    fn nav_left(&mut self, rng: &mut impl Rng) {
        self.set_ctrl(false, rng);
        self.set_shift(false, rng);
        self.press_key(KEY_LEFT, rng);
    }

    fn nav_right(&mut self, rng: &mut impl Rng) {
        self.set_ctrl(false, rng);
        self.set_shift(false, rng);
        self.press_key(KEY_RIGHT, rng);
    }

    fn nav_word_left(&mut self, rng: &mut impl Rng) {
        self.set_ctrl(true, rng);
        self.set_shift(false, rng);
        self.press_key(KEY_LEFT, rng);
    }

    fn nav_word_right(&mut self, rng: &mut impl Rng) {
        self.set_ctrl(true, rng);
        self.set_shift(false, rng);
        self.press_key(KEY_RIGHT, rng);
    }

    fn backspace(&mut self, rng: &mut impl Rng) {
        self.set_ctrl(false, rng);
        self.set_shift(false, rng);
        self.press_key(KEY_BACKSPACE, rng);
    }
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '\'' || c == '’'
}

fn apply_case_style(template: &str, lower: &str) -> String {
    if template.chars().all(|c| c.is_ascii_uppercase()) {
        return lower.to_ascii_uppercase();
    }
    let mut chars = template.chars();
    let first_is_upper = chars
        .next()
        .map(|c| c.is_ascii_uppercase())
        .unwrap_or(false);
    let rest_are_lower = chars.all(|c| !c.is_ascii_uppercase());

    if first_is_upper && rest_are_lower {
        let mut out = lower.to_string();
        if let Some(first) = out.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        return out;
    }

    lower.to_string()
}

fn synonym_options(word_lower: &str) -> &'static [&'static str] {
    match word_lower {
        "important" => &["crucial", "key", "vital"],
        "help" => &["assist", "aid", "support"],
        "use" => &["utilize", "employ"],
        "show" => &["demonstrate", "display"],
        "make" => &["create", "build"],
        "start" => &["begin", "kickoff"],
        "end" => &["finish", "wrap"],
        "idea" => &["concept", "notion"],
        "quick" => &["fast", "rapid"],
        "slow" => &["sluggish", "gradual"],
        _ => &[],
    }
}

fn word_variant(word: &str, rng: &mut impl Rng) -> Option<String> {
    let word_lower = word.to_ascii_lowercase();

    let options = synonym_options(word_lower.as_str());
    if !options.is_empty() {
        let option = options[rng.gen_range(0..options.len())];
        if option != word_lower {
            return Some(apply_case_style(word, option));
        }
    }

    if word_lower.ends_with("ed") && word_lower.len() >= 4 {
        let stem = &word_lower[..word_lower.len() - 2];
        return Some(apply_case_style(word, &format!("{stem}ing")));
    }

    if word_lower.ends_with("ing") && word_lower.len() >= 5 {
        let stem = &word_lower[..word_lower.len() - 3];
        return Some(apply_case_style(word, &format!("{stem}ed")));
    }

    None
}

fn word_typo(word: &str, rng: &mut impl Rng) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 2 {
        return None;
    }

    // Occasionally swap adjacent letters.
    if chars.len() >= 4 && rng.gen_bool(0.25) {
        let mut out = chars.clone();
        let idx = rng.gen_range(0..out.len() - 1);
        out.swap(idx, idx + 1);
        let out: String = out.into_iter().collect();
        if out != word {
            return Some(out);
        }
    }

    // Single-character substitution with a nearby key.
    let idx = rng.gen_range(0..chars.len());
    let mut out = chars.clone();
    if let Some(adj) = qwerty_adjacent_char(out[idx], rng) {
        out[idx] = adj;
        let out: String = out.into_iter().collect();
        if out != word {
            return Some(out);
        }
    }

    None
}

fn inter_char_delay_ms(wpm: f64, rng: &mut impl Rng) -> u64 {
    // Approximate 5 chars per word.
    let mean = 12000.0 / wpm;
    let stddev = mean * 0.35;
    let dist = Normal::new(mean, stddev.max(1.0)).unwrap();
    let sample = dist.sample(rng);
    sample.clamp(25.0, 900.0).round() as u64
}

fn punctuation_pause_ms(c: char, rng: &mut impl Rng) -> u64 {
    match c {
        ',' | ';' | ':' => rng.gen_range(60..=220),
        '.' | '!' | '?' => rng.gen_range(120..=520),
        '\n' => rng.gen_range(200..=900),
        _ => 0,
    }
}

fn maybe_think_pause_ms(prev: char, rng: &mut impl Rng) -> u64 {
    match prev {
        '.' | '!' | '?' => {
            if rng.gen_bool(0.12) {
                rng.gen_range(700..=2400)
            } else {
                0
            }
        }
        '\n' => {
            if rng.gen_bool(0.10) {
                rng.gen_range(600..=2000)
            } else {
                0
            }
        }
        _ => 0,
    }
}

fn byte_index_to_line_col(text: &str, byte_idx: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, c) in text.char_indices() {
        if i >= byte_idx {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn sentence_or_paragraph_boundary(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '\n')
}

#[derive(Debug, Clone)]
struct PhraseSpan {
    start: usize,
    original: String,
    alternative: String,
    original_len_chars: usize,
}

fn paragraph_byte_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut spans = Vec::new();
    let mut idx = 0usize;

    while idx < len {
        while idx < len && bytes[idx] == b'\n' {
            idx += 1;
        }
        if idx >= len {
            break;
        }

        let start = idx;
        while idx < len {
            if bytes[idx] == b'\n' && idx + 1 < len && bytes[idx + 1] == b'\n' {
                break;
            }
            idx += 1;
        }
        let end = idx;
        spans.push((start, end));

        while idx < len && bytes[idx] == b'\n' {
            idx += 1;
        }
    }

    spans
}

fn byte_index_to_char_index(text: &str, byte_idx: usize) -> usize {
    text[..byte_idx].chars().count()
}

fn phrase_spans_from_paragraph_alternatives(
    final_text: &str,
    alternatives_by_paragraph: &[Vec<PhraseAlternative>],
) -> Result<Vec<PhraseSpan>> {
    let paragraph_spans = paragraph_byte_spans(final_text);
    if alternatives_by_paragraph.len() != paragraph_spans.len() {
        return Err(anyhow!(
            "expected {} paragraph alternative lists, got {}",
            paragraph_spans.len(),
            alternatives_by_paragraph.len()
        ));
    }

    let final_text_len_chars = final_text.chars().count();
    let mut spans: Vec<PhraseSpan> = Vec::new();

    for (idx, (start_byte, end_byte)) in paragraph_spans.iter().copied().enumerate() {
        let paragraph = &final_text[start_byte..end_byte];
        let items = &alternatives_by_paragraph[idx];
        if let Err(err) = validate_phrase_alternatives(paragraph, items) {
            return Err(anyhow!(
                "phrase alternatives failed validation for paragraph {idx}: {err}"
            ));
        }

        for item in items {
            let local_start_byte = paragraph
                .find(&item.original)
                .ok_or_else(|| anyhow!("original not found in paragraph {idx}"))?;
            let global_start_byte = start_byte + local_start_byte;
            let start = byte_index_to_char_index(final_text, global_start_byte);
            let original_len_chars = item.original.chars().count();

            if start + original_len_chars > final_text_len_chars {
                return Err(anyhow!("phrase alternative out of bounds in final text"));
            }

            spans.push(PhraseSpan {
                start,
                original: item.original.clone(),
                alternative: item.alternative.clone(),
                original_len_chars,
            });
        }
    }

    spans.sort_by_key(|span| span.start);
    for window in spans.windows(2) {
        let prev_end = window[0].start + window[0].original_len_chars;
        let next_start = window[1].start;
        if prev_end > next_start {
            return Err(anyhow!("phrase alternative spans overlap in final text"));
        }
    }

    Ok(spans)
}

fn type_string(
    builder: &mut ActionBuilder,
    editor: &mut EditorState,
    s: &str,
    wpm: f64,
    rng: &mut impl Rng,
) -> Result<()> {
    for c in s.chars() {
        let stroke = keystroke_for_output_char(c).ok_or_else(|| {
            anyhow!(
                "unsupported character for US-QWERTY typing: {c:?} (U+{:04X})",
                c as u32
            )
        })?;
        builder.type_char(stroke, rng);
        editor.insert_char(c);

        let mut delay = inter_char_delay_ms(wpm, rng);
        delay += punctuation_pause_ms(c, rng);
        delay += maybe_think_pause_ms(c, rng);
        builder.wait(delay);
    }
    Ok(())
}

fn replace_at_end(
    builder: &mut ActionBuilder,
    editor: &mut EditorState,
    wrong: &str,
    correct: &str,
    wpm: f64,
    rng: &mut impl Rng,
) -> Result<()> {
    // Cursor must be at end of wrong.
    debug_assert!(editor.cursor == editor.buf.len());

    builder.wait(rng.gen_range(60..=260));

    let wrong_len = wrong.chars().count();
    for _ in 0..wrong_len {
        builder.backspace(rng);
        editor.backspace();
        builder.wait(rng.gen_range(15..=55));
    }

    type_string(builder, editor, correct, wpm, rng)
}

fn navigate_left_to(
    builder: &mut ActionBuilder,
    editor: &mut EditorState,
    target: usize,
    profile: WordNavProfile,
    rng: &mut impl Rng,
) {
    let target = target.min(editor.buf.len());

    match profile {
        WordNavProfile::Chrome => {
            while editor.cursor > target {
                let ctrl_target =
                    crate::word_nav::ctrl_left(&editor.buf, editor.cursor, is_word_char);
                let ctrl_delta = editor.cursor.saturating_sub(ctrl_target);
                let remaining = editor.cursor - target;
                let crosses_newline = editor.buf[ctrl_target..editor.cursor]
                    .iter()
                    .any(|c| *c == '\n');

                if ctrl_target >= target && ctrl_delta >= 4 && remaining >= 12 && !crosses_newline {
                    builder.nav_word_left(rng);
                    editor.move_word_left();
                } else {
                    builder.nav_left(rng);
                    editor.move_left();
                }

                if rng.gen_bool(0.03) {
                    builder.wait(rng.gen_range(40..=180));
                } else {
                    builder.wait(rng.gen_range(6..=22));
                }
            }
        }
        WordNavProfile::Compatible => {
            while editor.cursor > target {
                let ctrl_target =
                    crate::word_nav::ctrl_left(&editor.buf, editor.cursor, is_word_char);
                let ctrl_delta = editor.cursor.saturating_sub(ctrl_target);
                let remaining = editor.cursor - target;
                let safe_jump =
                    compatible_ctrl_jump_is_safe(&editor.buf, editor.cursor, ctrl_target);

                if ctrl_target >= target && ctrl_delta >= 4 && remaining >= 12 && safe_jump {
                    builder.nav_word_left(rng);
                    editor.move_word_left();
                } else {
                    builder.nav_left(rng);
                    editor.move_left();
                }

                if rng.gen_bool(0.03) {
                    builder.wait(rng.gen_range(40..=180));
                } else {
                    builder.wait(rng.gen_range(6..=22));
                }
            }

            builder.set_ctrl(false, rng);
        }
    }
}

fn navigate_right_to(
    builder: &mut ActionBuilder,
    editor: &mut EditorState,
    target: usize,
    profile: WordNavProfile,
    rng: &mut impl Rng,
) {
    let target = target.min(editor.buf.len());

    match profile {
        WordNavProfile::Chrome => {
            while editor.cursor < target {
                let ctrl_target =
                    crate::word_nav::ctrl_right(&editor.buf, editor.cursor, is_word_char);
                let ctrl_delta = ctrl_target.saturating_sub(editor.cursor);
                let remaining = target - editor.cursor;
                let crosses_newline = editor.buf[editor.cursor..ctrl_target]
                    .iter()
                    .any(|c| *c == '\n');

                if ctrl_target <= target && ctrl_delta >= 4 && remaining >= 12 && !crosses_newline {
                    builder.nav_word_right(rng);
                    editor.move_word_right();
                } else {
                    builder.nav_right(rng);
                    editor.move_right();
                }

                builder.wait(rng.gen_range(6..=22));
            }

            builder.set_ctrl(false, rng);
        }
        WordNavProfile::Compatible => {
            while editor.cursor < target {
                let ctrl_target =
                    crate::word_nav::ctrl_right(&editor.buf, editor.cursor, is_word_char);
                let ctrl_delta = ctrl_target.saturating_sub(editor.cursor);
                let remaining = target - editor.cursor;

                let safe_jump =
                    compatible_ctrl_jump_is_safe(&editor.buf, editor.cursor, ctrl_target);

                if ctrl_target <= target && ctrl_delta >= 4 && remaining >= 12 && safe_jump {
                    builder.nav_word_right(rng);
                    editor.move_word_right();
                } else {
                    builder.nav_right(rng);
                    editor.move_right();
                }

                builder.wait(rng.gen_range(6..=22));
            }

            builder.set_ctrl(false, rng);
        }
    }
}

fn fix_error_at_position(
    builder: &mut ActionBuilder,
    editor: &mut EditorState,
    err: OutstandingError,
    wpm: f64,
    profile: WordNavProfile,
    rng: &mut impl Rng,
) -> Result<()> {
    let wrong_len = err.wrong.chars().count();
    let target_end = err.start + wrong_len;
    if target_end > editor.cursor {
        return Err(anyhow!("internal error: correction target after cursor"));
    }

    navigate_left_to(builder, editor, target_end, profile, rng);

    builder.wait(rng.gen_range(50..=220));

    for _ in 0..wrong_len {
        builder.backspace(rng);
        editor.backspace();
        builder.wait(rng.gen_range(15..=55));
    }

    type_string(builder, editor, &err.correct, wpm, rng)?;

    // Return to end.
    navigate_right_to(builder, editor, editor.buf.len(), profile, rng);

    Ok(())
}

pub fn generate_plan_with_phrase_alternatives(
    final_text: &str,
    cfg: PlannerConfig,
    alternatives_by_paragraph: &[Vec<PhraseAlternative>],
    rng: &mut impl Rng,
) -> Result<Plan> {
    if let Some((byte_idx, c)) = find_first_unsupported_char(final_text) {
        let (line, col) = byte_index_to_line_col(final_text, byte_idx);
        return Err(anyhow!(
            "unsupported character {c:?} (U+{:04X}) at line {line}, column {col}. Supported: ASCII, newline, and smart quotes (’ ‘ ” “). Tabs are not allowed.",
            c as u32
        ));
    }

    let phrase_spans =
        phrase_spans_from_paragraph_alternatives(final_text, alternatives_by_paragraph)?;

    generate_plan_impl(final_text, cfg, &phrase_spans, rng)
}

pub fn generate_plan(final_text: &str, cfg: PlannerConfig, rng: &mut impl Rng) -> Result<Plan> {
    if cfg.no_revision {
        return generate_plan_no_revision(final_text, cfg, rng);
    }
    generate_plan_impl(final_text, cfg, &[], rng)
}

pub fn generate_plan_no_revision(
    final_text: &str,
    cfg: PlannerConfig,
    rng: &mut impl Rng,
) -> Result<Plan> {
    validate_config(&cfg)?;

    if let Some((byte_idx, c)) = find_first_unsupported_char(final_text) {
        let (line, col) = byte_index_to_line_col(final_text, byte_idx);
        return Err(anyhow!(
            "unsupported character {:?} (U+{:04X}) at line {}, column {}. Supported: ASCII, newline, and smart quotes. Tabs are not allowed.",
            c, c as u32, line, col
        ));
    }

    let keymap = us_qwerty_keymap()?;
    let wpm_target = rng.gen_range(cfg.wpm_min..=cfg.wpm_max);

    let mut builder = ActionBuilder::new(keymap.shift_mask, keymap.ctrl_mask);
    let mut editor = EditorState::default();

    builder.set_modifiers();
    builder.wait(rng.gen_range(250..=600));

    type_string(&mut builder, &mut editor, final_text, wpm_target, rng)?;

    builder.set_shift(false, rng);
    builder.set_ctrl(false, rng);
    builder.set_modifiers();

    let final_simulated = editor.as_string();
    if final_simulated != final_text {
        return Err(anyhow!(
            "planner bug: simulated text does not match final draft"
        ));
    }

    Ok(Plan {
        version: 1,
        config: PlanConfig {
            layout: keymap.layout,
            keymap_format: keymap.keymap_format,
            keymap: keymap.keymap,
            wpm_target,
        },
        actions: builder.into_actions(),
    })
}

fn generate_plan_impl(
    final_text: &str,
    cfg: PlannerConfig,
    phrase_spans: &[PhraseSpan],
    rng: &mut impl Rng,
) -> Result<Plan> {
    validate_config(&cfg)?;

    if let Some((byte_idx, c)) = find_first_unsupported_char(final_text) {
        let (line, col) = byte_index_to_line_col(final_text, byte_idx);
        return Err(anyhow!(
            "unsupported character {c:?} (U+{:04X}) at line {line}, column {col}. Supported: ASCII, newline, and smart quotes (’ ‘ ” “). Tabs are not allowed.",
            c as u32
        ));
    }

    let keymap = us_qwerty_keymap()?;

    let wpm_target = rng.gen_range(cfg.wpm_min..=cfg.wpm_max);

    let mut builder = ActionBuilder::new(keymap.shift_mask, keymap.ctrl_mask);
    let mut editor = EditorState::default();
    let mut outstanding: Vec<OutstandingError> = Vec::new();

    // Ensure compositor and clients start from a neutral modifier state.
    builder.set_modifiers();
    builder.wait(rng.gen_range(250..=600));

    let chars: Vec<char> = final_text.chars().collect();
    let mut i = 0usize;
    let mut phrase_idx = 0usize;
    let mut last_char: char;

    while i < chars.len() {
        let progress = (i as f64) / (chars.len() as f64);
        let next_phrase_start = phrase_spans.get(phrase_idx).map(|span| span.start);

        if next_phrase_start == Some(i) {
            let span = &phrase_spans[phrase_idx];
            let typed: &str;

            if outstanding.len() < cfg.max_outstanding_errors {
                let start_cursor = editor.cursor;
                typed = span.alternative.as_str();
                type_string(&mut builder, &mut editor, typed, wpm_target, rng)?;
                outstanding.push(OutstandingError {
                    start: start_cursor,
                    wrong: span.alternative.clone(),
                    correct: span.original.clone(),
                    fix_after_chars: rng.gen_range(90..=420),
                    constraint: CorrectionConstraint::SentenceOrParagraphBoundary,
                });
            } else {
                typed = span.original.as_str();
                type_string(&mut builder, &mut editor, typed, wpm_target, rng)?;
            }

            last_char = typed
                .chars()
                .last()
                .ok_or_else(|| anyhow!("phrase alternative must not be empty"))?;

            i += span.original_len_chars;
            phrase_idx += 1;
        } else if is_word_char(chars[i]) {
            let start = i;
            i += 1;
            while i < chars.len() && is_word_char(chars[i]) {
                i += 1;
            }
            let word_end = i;

            if let Some(p) = next_phrase_start {
                if p > start && p < word_end {
                    let prefix: String = chars[start..p].iter().collect();
                    type_string(&mut builder, &mut editor, &prefix, wpm_target, rng)?;
                    last_char = chars[p - 1];
                    i = p;
                } else {
                    let word: String = chars[start..word_end].iter().collect();

                    let inject_error = rng.gen_bool(cfg.error_rate_per_word)
                        && outstanding.len() < cfg.max_outstanding_errors;

                    if inject_error {
                        let want_variant = rng.gen_bool(cfg.word_variant_share);
                        let wrong = if want_variant {
                            word_variant(&word, rng).or_else(|| word_typo(&word, rng))
                        } else {
                            word_typo(&word, rng).or_else(|| word_variant(&word, rng))
                        };

                        if let Some(wrong_word) = wrong {
                            let word_start_cursor = editor.cursor;
                            type_string(&mut builder, &mut editor, &wrong_word, wpm_target, rng)?;

                            if rng.gen_bool(cfg.immediate_fix_rate) {
                                replace_at_end(
                                    &mut builder,
                                    &mut editor,
                                    &wrong_word,
                                    &word,
                                    wpm_target,
                                    rng,
                                )?;
                            } else {
                                outstanding.push(OutstandingError {
                                    start: word_start_cursor,
                                    wrong: wrong_word,
                                    correct: word,
                                    fix_after_chars: rng.gen_range(25..=220),
                                    constraint: CorrectionConstraint::None,
                                });
                            }
                        } else {
                            type_string(&mut builder, &mut editor, &word, wpm_target, rng)?;
                        }
                    } else {
                        type_string(&mut builder, &mut editor, &word, wpm_target, rng)?;
                    }

                    last_char = chars[word_end - 1];
                }
            } else {
                let word: String = chars[start..word_end].iter().collect();

                let inject_error = rng.gen_bool(cfg.error_rate_per_word)
                    && outstanding.len() < cfg.max_outstanding_errors;

                if inject_error {
                    let want_variant = rng.gen_bool(cfg.word_variant_share);
                    let wrong = if want_variant {
                        word_variant(&word, rng).or_else(|| word_typo(&word, rng))
                    } else {
                        word_typo(&word, rng).or_else(|| word_variant(&word, rng))
                    };

                    if let Some(wrong_word) = wrong {
                        let word_start_cursor = editor.cursor;
                        type_string(&mut builder, &mut editor, &wrong_word, wpm_target, rng)?;

                        if rng.gen_bool(cfg.immediate_fix_rate) {
                            replace_at_end(
                                &mut builder,
                                &mut editor,
                                &wrong_word,
                                &word,
                                wpm_target,
                                rng,
                            )?;
                        } else {
                            outstanding.push(OutstandingError {
                                start: word_start_cursor,
                                wrong: wrong_word,
                                correct: word,
                                fix_after_chars: rng.gen_range(25..=220),
                                constraint: CorrectionConstraint::None,
                            });
                        }
                    } else {
                        type_string(&mut builder, &mut editor, &word, wpm_target, rng)?;
                    }
                } else {
                    type_string(&mut builder, &mut editor, &word, wpm_target, rng)?;
                }

                last_char = chars[word_end - 1];
            }
        } else {
            let c = chars[i];
            i += 1;

            // Occasional double-space typo.
            if c == ' ' && rng.gen_bool(0.015) && outstanding.len() < cfg.max_outstanding_errors {
                let start_cursor = editor.cursor;
                type_string(&mut builder, &mut editor, "  ", wpm_target, rng)?;
                outstanding.push(OutstandingError {
                    start: start_cursor,
                    wrong: "  ".to_string(),
                    correct: " ".to_string(),
                    fix_after_chars: rng.gen_range(40..=260),
                    constraint: CorrectionConstraint::None,
                });
            } else {
                type_string(&mut builder, &mut editor, &c.to_string(), wpm_target, rng)?;
            }

            last_char = c;
        }

        // Occasionally fix a recent mistake (delayed correction).
        if let Some(err) = outstanding.last() {
            let wrong_len = err.wrong.chars().count();
            let age = editor.cursor.saturating_sub(err.start + wrong_len);
            let late_stage = progress >= cfg.stop_corrections_after_progress;

            let force_fix = outstanding.len() >= cfg.max_outstanding_errors;
            let due = age >= err.fix_after_chars;

            let boundary_for_random_fix = match err.constraint {
                CorrectionConstraint::None => last_char == ' ' || ",.;:!?\n".contains(last_char),
                CorrectionConstraint::SentenceOrParagraphBoundary => {
                    sentence_or_paragraph_boundary(last_char)
                }
            };

            let random_fix = !late_stage && rng.gen_bool(0.12) && boundary_for_random_fix;

            let should_fix = match err.constraint {
                CorrectionConstraint::None => force_fix || (due && !late_stage) || random_fix,
                CorrectionConstraint::SentenceOrParagraphBoundary => {
                    sentence_or_paragraph_boundary(last_char)
                        && (force_fix || (due && !late_stage) || random_fix)
                }
            };

            if should_fix {
                let err = outstanding.pop().unwrap();
                fix_error_at_position(
                    &mut builder,
                    &mut editor,
                    err,
                    wpm_target,
                    cfg.word_nav_profile,
                    rng,
                )?;
                builder.wait(rng.gen_range(80..=420));
            }
        }
    }

    // Always do a near-end review pass.
    builder.wait(rng.gen_range(cfg.review_pause_ms_min..=cfg.review_pause_ms_max));

    while let Some(err) = outstanding.pop() {
        fix_error_at_position(
            &mut builder,
            &mut editor,
            err,
            wpm_target,
            cfg.word_nav_profile,
            rng,
        )?;
        builder.wait(rng.gen_range(120..=520));
    }

    // Return to neutral modifiers.
    builder.set_shift(false, rng);
    builder.set_ctrl(false, rng);
    builder.set_modifiers();

    let final_simulated = editor.as_string();
    if final_simulated != final_text {
        return Err(anyhow!(
            "planner bug: simulated text does not match final draft"
        ));
    }

    Ok(Plan {
        version: 1,
        config: PlanConfig {
            layout: keymap.layout,
            keymap_format: keymap.keymap_format,
            keymap: keymap.keymap,
            wpm_target,
        },
        actions: builder.into_actions(),
    })
}
