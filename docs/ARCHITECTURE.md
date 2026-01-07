# Architecture

## High-level overview

`drafter` has two distinct phases:

1. **Plan**: turn a final draft text into a fully-expanded sequence of low-level keyboard actions (key presses/releases, modifier updates, and waits).
2. **Play**: connect to Wayland, create a virtual keyboard, set its keymap, then replay the precomputed action sequence into the currently focused surface.

This separation is intentional:

- Planning can be done offline and inspected/debugged.
- Playback is simple and timing-focused.
- The “final editor text must match the final draft” requirement is enforced by the planner’s internal simulation, not by reading the editor.

Behavioral requirements live in `docs/typing-behavior-requirements.md`.

## Repository map

- `src/main.rs` — CLI (`plan`, `play`, `run`).
- `src/planner.rs` — plan generation (human-like behavior + internal verification).
- `src/playback.rs` — Wayland virtual keyboard playback (+ optional console trace output).
- `src/trace.rs` — derive high-level `Typing`/`Replace` trace lines from low-level actions.
- `src/model.rs` — `Plan` / `Action` types (JSON).
- `src/keyboard.rs` — keycode constants + character→keystroke mapping.
- `src/keymap.rs` — XKB keymap generation (US layout).
- `src/protocols.rs` + `protocol/virtual-keyboard-unstable-v1.xml` — Wayland protocol bindings via `wayland-scanner`.
- `tests/` — planner-focused tests.

## Planning algorithm

The plan phase takes a final draft and produces a complete, precomputed sequence of keyboard actions that will (when replayed) end with the editor containing the final draft exactly.

The planner follows this general approach:

Throughout the process it maintains a **plan-under-construction**: a single ordered list of keyboard actions (keys, modifier updates, and waits), plus a small amount of “current input state” (for example, whether Shift/Ctrl are currently held). Each step appends more actions to the end of this list as decisions are made. When the planner decides to correct an earlier mistake, it appends the entire correction subsequence (cursor movement → deletion → retype → return to original position) at that moment.

In the current codebase, this plan-under-construction is the `Plan`/`Action` list produced by the planner, but the underlying idea is language-agnostic.

1. **Validate and normalize the draft**
   - Ensure the draft is plain text and only contains characters the tool knows how to type.
   - Fail early with a precise location for any unsupported characters.
   - Handle a small set of common “smart quotes” by typing their ASCII equivalents and relying on editor auto-substitution.

2. **Choose run parameters**
   - Pick a target typing speed within a configured range.
   - Pick randomness-driven choices such as when to pause, how many errors to introduce, and how long to wait between keys.

3. **Turn the draft into a stream of units to type**
   - Walk through the draft in order, identifying words versus separators (spaces, punctuation, newlines).
   - This gives natural opportunities to insert typos/variations and to schedule “thinking” pauses.

4. **Emit low-level key actions for forward typing**
   - For each character, emit key press/release events and a wait before the next character.
   - Add additional waits around punctuation and newlines to mimic human rhythm.

5. **Introduce divergences intentionally**
   - Occasionally type something that does not match the final draft (a typo or a small word variation).
   - Record each divergence as an outstanding issue: where it occurred, what was typed, and what the correct text should be.

6. **Interleave corrections**
   - Some issues are fixed immediately (backspace and retype right away).
   - Others are fixed later: the plan includes cursor movement back to the earlier location, deletion of the wrong text, typing of the corrected text, and then returning to the original typing position.

7. **Near-end review pass**
   - After the forward pass finishes, the planner always performs a review pause and then fixes all remaining outstanding issues.

8. **Verify correctness without reading the editor**
   - Throughout planning, the planner maintains an internal model of the editor buffer and cursor.
   - The plan is accepted only if applying all planned actions to this internal model yields exactly the final draft.

The planner is responsible for these decisions; playback is a simple, timing-focused replay of the already-decided actions.

## Data flow

- Input text (`draft.txt`) → `planner::generate_plan()` → `model::Plan` (serializable JSON)
- `model::Plan` → `playback::play_plan()` → Wayland `zwp_virtual_keyboard_v1` events

At runtime, `drafter` always assumes the editor is already focused and ready for insertion at the caret.

## Capabilities and algorithms

This section describes the human-like typing/editing behaviors `drafter` can emulate, and the general algorithm used to implement each behavior.

### Supported capabilities

- **Character-by-character typing**
  - Algorithm: the planner emits a `Key(pressed)` + short hold + `Key(released)` for every character, plus a `Wait` between characters.

- **Variable typing speed (~40–60 WPM with jitter)**
  - Algorithm: pick a target WPM within `wpm_min..=wpm_max`, then sample a per-character delay from a distribution around `mean_ms = 12000 / wpm` (≈ 5 chars/word) and clamp to a human-ish range.

- **Micro-pauses and “thinking” pauses**
  - Algorithm: add small extra delays after punctuation and newlines, plus occasional longer pauses at sentence/paragraph boundaries.

- **Intentional typos**
  - Algorithm: per word, probabilistically inject a typo using:
    - adjacent-key substitutions (US-QWERTY neighbor map)
    - occasional adjacent-letter swaps
    - occasional double-space insertion

- **Small word/phrase variations**
  - Algorithm: sometimes replace a word with a simple variant (synonym table + limited tense swaps). Optionally, the planner can also replace longer spans using paragraph-local `PhraseAlternative` suggestions; these are treated as “wrong for now” and are later corrected back to the final draft (with phrase-level fixes biased toward sentence/paragraph boundaries).

- **Immediate micro-edits (type → fix right away)**
  - Algorithm: after typing a wrong word, backspace the just-typed word and retype the correct one.

- **Delayed corrections (type wrong now → fix later)**
  - Algorithm:
    1. While typing, store `OutstandingError { start, wrong, correct, fix_after_chars, constraint }`.
    2. Later (based on age/pressure/randomness), navigate left back to the end of the wrong span using a mix of `Left` and `Ctrl+Left`, then fine-tune with `Left`.
    3. Backspace the wrong span, type the correct span, then navigate right back using a mix of `Right` and `Ctrl+Right`.

- **Word navigation (Ctrl+Left/Right)**
  - Algorithm: during corrections, the planner may use word-jump shortcuts depending on a selectable word navigation profile:
    - `chrome` (default): current behavior tuned to match Chrome/Docs word-boundary semantics.
    - `compatible`: conservative mode; only emits `Ctrl+Left/Right` when the predicted jump stays within simple ASCII words+spaces (and is not adjacent to punctuation), otherwise falls back to plain `Left/Right`.

- **Near-end review pass (always)**
  - Algorithm: after finishing the forward typing pass, insert a review pause and then fix all remaining outstanding errors.

- **Keyboard-only interaction with safe keys**
  - Algorithm: plans are composed only of low-level key events and modifier updates; the current planner uses printable characters, `Enter`, arrows, `Backspace`, and `Ctrl+Left/Right`.

- **Smart quotes in the final draft (`’‘”“`)**
  - Algorithm: the planner tracks the Unicode characters in the final draft, but emits ASCII keystrokes (`'` and `"`) and relies on editor auto-substitution (e.g. Google Docs smart quotes) so the final editor text can match the draft.

### Not yet supported

- **General Unicode typing** (beyond `’‘”“`) and **non-US keyboard layouts**.
- **Selection-based editing** (Shift+arrows, Shift+Home/End) and **word deletion shortcuts** (Ctrl+Backspace/Delete).
- **Undo/redo-driven correction strategies**.
- **Starting-state management** (e.g. clearing an existing document) and **any reading/verification of editor contents**.
- **Editor-aware behavior** (reacting to spellcheck/autocorrect, or different keybindings per editor).
- **Rich-text formatting** (intentionally out of scope).

## Major abstractions and modules

### `Plan` and `Action` (`src/model.rs`)

`Plan` is the on-disk and in-memory representation of “everything that will happen”.

- `Plan.config` includes the keymap string and basic planning parameters.
- `Plan.actions` is an ordered list of low-level actions:
  - `Action::Wait { ms }`
  - `Action::Modifiers { mods_depressed, mods_latched, mods_locked, group }`
  - `Action::Key { keycode, state }`

Keeping actions low-level makes playback backend-agnostic and keeps the “precompute everything” requirement straightforward.

### Keyboard mapping helpers (`src/keyboard.rs`)

Provides:

- Linux evdev keycode constants for the keys this project uses.
- `char_to_keystroke()` for US-QWERTY ASCII mapping.
- `typed_char_for_output_char()` / `keystroke_for_output_char()` which define what the tool can produce.

Smart quotes support:

- Drafts may contain `’‘”“`.
- These are mapped to ASCII `'` and `"` keystrokes, relying on editor auto-substitution.
- Internally, the planner still tracks the intended final draft characters.

### XKB keymap generation (`src/keymap.rs`)

`us_qwerty_keymap()` constructs an XKB keymap string (`KEYMAP_FORMAT_TEXT_V1`) for rules/model/layout `evdev/pc105/us` and also returns modifier bit masks.

This keymap string is sent to the compositor via `zwp_virtual_keyboard_v1.keymap()`, enabling consistent interpretation of the evdev keycodes.

### Planner (`src/planner.rs`)

The planner is responsible for “human-like behavior” while ensuring the final result matches the draft.

Key responsibilities:

- **Validation**: rejects unsupported characters early and reports line/column.
- **Timing model**:
  - per-character delays derived from a WPM target
  - micro-pauses at punctuation/newlines
  - occasional longer “thinking” pauses
- **Error injection**:
  - character-level typos (adjacent-key substitutions, swaps)
  - small word-level variants (synonyms / tense tweaks)
- **Corrections**:
  - immediate fixes (type wrong → backspace → retype)
  - delayed fixes (move cursor left, backspace, retype, move back to end)
  - always runs a near-end “review pass” that fixes remaining outstanding errors

To make this feasible without reading the editor, the planner maintains an internal `EditorState` (buffer + cursor) and applies the planned edits to it. The planner verifies that `EditorState` equals the final draft at the end.

### LLM Helper (`src/llm.rs`) [Experimental]

An optional module (enabled via the `llm` feature) that interacts with remote Large Language Models (specifically via OpenRouter) to generate phrasing alternatives.

- **Goal**: Propose "wrong" alternative phrases that mean the same thing, allowing the planner to type a variation and later correct it back to the original.
- **Data flow**: Draft paragraphs → OpenRouter API → `Vec<Vec<PhraseAlternative>>` → `planner::generate_plan_with_phrase_alternatives()`.
- **Constraints**: Enforces strict validation (unique substring, non-overlapping, safe characters) to ensure the planner can deterministically locate and replace the text; phrase-level corrections are restricted to sentence/paragraph boundaries during the forward typing pass.

### Wayland protocol bindings (`src/protocols.rs`, `protocol/virtual-keyboard-unstable-v1.xml`)

Wayland bindings for `virtual-keyboard-unstable-v1` are generated at compile time using `wayland-scanner` from the XML in `protocol/virtual-keyboard-unstable-v1.xml`.

This avoids depending on external protocol packages at runtime.

### Playback (`src/playback.rs`)

Playback:

- Connects to Wayland and binds:
  - `wl_seat`
  - `zwp_virtual_keyboard_manager_v1`
- Creates a `zwp_virtual_keyboard_v1` tied to the seat.
- Sends the XKB keymap via `keymap()`.
- Replays each action:
  - `Wait` → sleeps
  - `Modifiers` → sends `zwp_virtual_keyboard_v1.modifiers()`
  - `Key` → sends `zwp_virtual_keyboard_v1.key()` with a monotonic “time since start” timestamp
- Optionally prints a high-level console trace derived from the action stream (enabled by default; disable with `--no-trace`).

A Ctrl+C handler is installed to abort playback and attempt to reset modifiers.

### CLI (`src/main.rs`)

Implements three commands:

- `plan`: read draft → generate plan → write JSON
- `play`: read JSON → replay
- `run`: plan then play

CLI is intentionally thin; most logic is in the planner and playback modules.

### Stats (`src/sim.rs`)

Provides lightweight plan statistics (action count, key events, total wait time) for UX feedback, plus `simulate_typed_text()` which applies a plan to a simple editor model for tests/debugging.

`simulate_typed_text()` models basic insertion, left/right cursor movement, and backspace/delete. It does not model editor-specific behavior such as smart-quote auto-substitution.

## Testing

- `tests/planner_roundtrip.rs` exercises planner behavior and includes a regression test for smart apostrophes.
- `tests/planner_phrase_alternatives.rs` verifies phrase alternatives are typed and then corrected so the final output matches the input exactly.
- `tests/llm_validation.rs` covers `llm::validate_phrase_alternatives()` with non-network cases.

## Known limitations (by design)

- The tool does not read the editor contents; you must start from an empty/known state.
- Smart quotes require editor auto-substitution to match the final draft exactly.
- Edits are biased toward recent text to avoid large cursor navigation for multi-page drafts.
