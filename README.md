# drafter

`drafter` is a Linux/Wayland typing simulator: given a “final draft” text, it produces a human-like stream of keyboard events (variable speed, pauses, mistakes, edits, and later corrections) so that an editor such as Google Docs ends up with the final draft.

This project targets Sway (wlroots) and uses the Wayland `virtual-keyboard-unstable-v1` protocol (compositor-protocol-only; no `uinput`).

## What it does

- Types character-by-character with human-like timing (~40–60 WPM average, with jitter)
- Injects occasional divergences (typos and small word-level variations)
- Fixes some immediately and others later
- Always performs a near-end “review pass” to fix remaining issues
- Emits keyboard events only (no clipboard)

Behavior is guided by `docs/typing-behavior-requirements.md`. Architecture and planning internals are in `docs/ARCHITECTURE.md`.

## Requirements

- Linux + Wayland session
- Sway compositor
- Sway/wlroots must expose `zwp_virtual_keyboard_manager_v1` to clients (the tool errors if the protocol isn’t advertised)
- A US-QWERTY layout is assumed for keystroke mapping

System libraries for building:

- `libwayland-client`
- `libxkbcommon`

## Build

```bash
cargo build --release
```

To enable LLM support (for alternative phrasing):

```bash
cargo build --release --features llm
```

## Usage

`drafter` types into **whatever is currently focused**. It does not read the editor contents.

General workflow:

1. Open your target editor (e.g. Google Docs).
2. Make sure the document is empty (or at least the cursor is where you want text inserted).
3. Place the caret where typing should begin (click into the document).
4. Run `drafter` with a short countdown.
5. During the countdown, do not touch the keyboard/mouse; let it type.
6. Press `Ctrl+C` to abort.

By default, `play`/`run` print a live trace of typing/corrections to stderr (this includes draft text). Use `--no-trace` to disable.

### One-shot: plan then play

```bash
./target/release/drafter run --input draft.txt --countdown 5
```

### Two-step: plan and play separately

Generate a plan JSON:

```bash
./target/release/drafter plan --input draft.txt --output plan.json
```

Play it into the focused editor:

```bash
./target/release/drafter play --plan plan.json --countdown 5
```

### Tuning

- `--wpm-min` / `--wpm-max`: speed range
- `--error-rate`: probability of injecting an error per word
- `--immediate-fix-rate`: how often an error is fixed immediately
- `--profile <chrome|compatible>`: word navigation behavior used during corrections (default `chrome`)
- `--seed`: make planning deterministic (useful for debugging)
- `--no-trace`: disable console typing/correction trace during playback (on by default)

### Experimental LLM phrasing

When built with `--features llm`, `plan` and `run` can request paragraph-local phrase alternatives from OpenRouter, temporarily type them, and later edit them back so the final text matches the input exactly.

- `--llm`: enable OpenRouter suggestions
- `--llm-model <MODEL>`: model name (default `google/gemini-3-flash-preview`)
- `--llm-max-suggestions <N>`: maximum suggestions per paragraph
- `--llm-rewrite-strength <subtle|moderate|dramatic>`
- `--llm-max-concurrency <1-10>`
- `--llm-cache <PATH>`: read/write JSON cache (contains your draft text); if it exists, no network is used
- `--llm-on-error <fallback|error>`: default `fallback`

Fetching requires building with `--features llm`. Without it, `--llm-cache` must already exist.
Requires `OPENROUTER_API_KEY` in the environment (loads `.env` if present).

Example:

```bash
./target/release/drafter run --input draft.txt --countdown 5 --llm --llm-cache llm.json --seed 123
```

## Text limitations

- Plain text only.
- Tabs are not supported.
- ASCII is supported.
- “Smart quotes” characters `’‘”“` are accepted in the draft:
  - The tool types ASCII `'` and `"` and relies on editor auto-substitution (e.g. Google Docs smart quotes) to produce the Unicode punctuation.
  - If smart quotes are disabled in your editor, replace these characters in the draft with plain ASCII.

## Troubleshooting

- `zwp_virtual_keyboard_manager_v1 not available`:
  - Your compositor session isn’t exposing the protocol to clients.
  - You can check advertised globals with `wayland-info` (package `wayland-utils`).

- Output doesn’t match the draft:
  - The editor wasn’t empty when you started.
  - Focus changed during playback.
  - Smart quotes were disabled (see “Text limitations”).

## Safety notes

This tool generates real keyboard events. Use it carefully:

- Close other windows and focus the correct editor.
- Don’t run it on a system where unintended keystrokes could be destructive.
