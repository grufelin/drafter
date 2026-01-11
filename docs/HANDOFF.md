# drafter

`drafter` is a Linux/Wayland typing simulator: given a “final draft” plain-text input, it **plans** a full sequence of human-like keyboard events and then **plays** them into the currently focused editor so the editor ends up matching the provided draft.

This file is meant to give new agents enough context to explore the repo safely and effectively.

## Big picture

`drafter` has two phases:

1. **Plan**: Convert the final draft text into a fully precomputed list of low-level keyboard actions (`Key` press/release, `Modifiers`, and `Wait`).
2. **Play**: Connect to Wayland, create a virtual keyboard (`virtual-keyboard-unstable-v1`), set an XKB keymap, then replay the planned actions into the focused surface.

The tool never reads editor contents; correctness is enforced by the planner’s internal simulation.

## Workflow

Start here with key docs here, which can help you undertand the project and explore codebase easier:

- `docs/ARCHITECTURE.md` — architecture, module map, planner algorithm, playback overview
- `docs/typing-behavior-requirements.md` — key allowlist and behavioral constraints

For major jobs, create and keep a short working note in `docs/tasks/`:

- One file per job: `docs/tasks/YYYY-MM-DD-<short-slug>.md`
- Update this log with your observations, thinking, and action as you proceed through the job: goal, current state, , approach, decisions, progress, etc.

### Interpreting user requests
- If the user says **“update doc”**, update the relevant parts of all relevant docs/files (inside and outside `docs/`) so everything stays consistent.
  - CLI options / CLI usage: update `README.md`.
  - Architecture changes: update `docs/ARCHITECTURE.md`.
  - Key allowlist and behavioral constraints changes: update `docs/typing-behavior-requirements.md.md`.
  - Technical requirements (coding styles, library usage, dev tools): update `AGENTS.md` (keep it very concise).
- If the user says **“commit”**, make small, focused commits grouping related files.
- If the user says **“prepare to write the handoff document**, propose a few next tasks; after the user approves, update this documeent (`docs/HANDOFF.md`) with fresh context + the agreed next tasks for the next session.

## Non-negotiable safety constraints

These are core to the project’s design:

- **Keyboard events only**: playback must only emit keyboard events; no mouse, no clipboard.
- **Never read editor contents**: do not add code that inspects the focused window’s text.
- **No clipboard operations**: do not read from or write to clipboard; no copy/cut/paste.
- **Safe-key policy**: only emit allowed keys/shortcuts (see `docs/typing-behavior-requirements.md`).
- **Precompute before playback**: planning must fully precompute the action sequence before sending events.

Treat drafts as potentially sensitive:

- Avoid logging draft text.
- The console trace feature prints portions of typed/corrected text; users can disable it with `--no-trace`.
- LLM cache files contain draft text.

## What not to do

- Don’t add features that read the screen, capture keystrokes, scrape window contents, or interact with the clipboard.
- Don’t add selection-based editing or other shortcuts unless they are explicitly allowed and documented.
- Don’t broaden scope beyond keyboard-only playback without a clear design review.
