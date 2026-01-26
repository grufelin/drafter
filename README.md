# drafter

`drafter` is a Linux typing simulator: given a “final draft” text, it produces a human-like stream of keyboard events (variable speed, pauses, mistakes, edits, and later corrections) so that a text input area (such as an editor) ends up with the final draft.

Playback supports Wayland (via `virtual-keyboard-unstable-v1`) and X11 (via the XTEST extension). It emits keyboard events only (no clipboard, no reading editor contents).

## Usage

### Basic

`drafter` types into whatever is currently focused. It does not read editor contents.

General workflow:

1. Open your target editor (and close/blur anything sensitive).
2. Make sure the insertion point is where you want typing to begin.
3. Run `drafter` with a short countdown. The simplest end-to-end command is `run` (plan then play):

```bash
drafter run --input draft.txt --countdown 5
```

4. Do not touch the keyboard/mouse during playback. If needed, press `Ctrl+C` to abort.

You can also read the draft from stdin:

```bash
cat draft.txt | drafter run --input - --countdown 5
```

If you want to keep the plan for later reuse, use two steps:

```bash
drafter plan --input draft.txt --output plan.json
drafter play --plan plan.json --countdown 5
```

### Advanced

Pick a playback backend (useful in Wayland sessions with Xwayland). `auto` prefers Wayland when both are available:

```bash
drafter run --input draft.txt --backend auto
drafter run --input draft.txt --backend wayland
drafter run --input draft.txt --backend x11
```

(`--backend` applies to `play` and `run`.)

Tune typing behavior:

- Speed: `--wpm-min` / `--wpm-max`
- Error injection: `--error-rate` and `--immediate-fix-rate` (set `--error-rate 0` for straight-through typing with no revisions)
- Cursor-word navigation: `--profile <chrome|compatible>`
- Determinism for debugging: `--seed <N>`

Control timing and outputs:

- Countdown before playback: `--countdown <secs>`
- Save the generated plan in `run`: `--output plan.json`

Wayland seat selection (Wayland only):

```bash
drafter run --input draft.txt --seat seat0
```

By default, `play` and `run` print a live trace of typing and corrections to stderr (this includes draft text). Disable it with `--no-trace`:

```bash
drafter run --input draft.txt --no-trace
```

LLM phrasing: With the `llm` feature enabled, `plan` and `run` can request paragraph-local phrase alternatives from OpenRouter, temporarily type them, and later edit them back so the final text matches the input exactly.

```bash
drafter run --input draft.txt --llm
```

LLM notes:

- Requires `OPENROUTER_API_KEY` in the environment (loads `.env` if present).
- `--llm` is incompatible with `--error-rate 0`.

## Development

Default features enable both Wayland and X11 playback; LLM support is opt-in.

You need a recent Rust toolchain (edition 2021) plus the system libraries listed below.

Build:

```bash
cargo build
```

Feature selection examples:

```bash
# X11-only build
cargo build --no-default-features --features x11

# Wayland-only build
cargo build --no-default-features --features wayland

# Enable LLM support in addition to the default backends
cargo build --features llm
```

System dependencies:

- `libxkbcommon` is used for keymap generation (planner) and is required for all builds.
- `libwayland-client` is required when building with the `wayland` feature (enabled by default).

Runtime environments:

- Wayland playback requires a compositor that exposes `zwp_virtual_keyboard_manager_v1` to clients (this project is primarily tested on Sway/wlroots).
- X11 playback requires an X server with the XTEST extension and currently assumes the X server keymap is US-QWERTY (the backend will validate and suggest `setxkbmap us` if it does not match).

Tests:

```bash
cargo test
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

- `X11 backend requires the XTEST extension`:
  - Your X server does not expose XTEST (or it’s blocked). Try a different Xorg/Xwayland setup.

- `X11 backend currently requires a US keyboard layout`:
  - Set your X keymap to US (example: `setxkbmap us`).

- Output doesn’t match the draft:
  - The editor wasn’t empty when you started.
  - Focus changed during playback.
  - Smart quotes were disabled (see “Text limitations”).

## Safety notes

This tool generates real keyboard events. Use it carefully:

- Close other windows and focus the correct editor.
- Don’t run it on a system where unintended keystrokes could be destructive.
