# drafter

`drafter` is a Linux typing simulator for Wayland and X11: given a “final draft” plain-text input, it **plans** a full sequence of human-like keyboard events and then **plays** them into the currently focused editor so the editor ends up matching the provided draft.

This file is meant to give new agents enough context to explore the repo safely and effectively.

## Big picture

`drafter` has two phases:

1. **Plan**: Convert the final draft text into a fully precomputed list of low-level keyboard actions (`Key` press/release, `Modifiers`, and `Wait`).
2. **Play**: Replay the planned actions into the currently focused surface using either:
   - **Wayland**: `virtual-keyboard-unstable-v1` virtual keyboard + XKB keymap.
   - **X11**: XTEST synthetic `KeyPress`/`KeyRelease` events (requires XTEST and a compatible server keymap).

The tool never reads editor contents; correctness is enforced by the planner’s internal simulation.

## Workflow

- For major jobs, at the start of a session, create a new task log in `docs/tasks/` named `YYYY-MM-DD_HHMM_<slug>.md`. As you research and work, keep the task log updated with what you explore/consider/plan/implement.

- The consult these documents; they will help you have an easier understand the project and pull the right context:
  - `docs/ARCHITECTURE.md` — architecture, module map, planner algorithm, playback overview
  - `docs/typing-behavior-requirements.md` — key allowlist and behavioral constraints

- When the user asks you to update documentation (only do this when asked), use this guide to decide what to update:
  - CLI options / CLI usage: update `README.md`.
  - Architecture changes: update `docs/ARCHITECTURE.md`.
  - Key allowlist and behavioral constraints changes: update `docs/typing-behavior-requirements.md.md`.
  - Technical requirements (stack, tooling, testing, building, coding styles, etc.): update `AGENTS.md` (keep it very concise).
- When the user asks you to make commits, make small, focused commits grouping related files.
- When the user asks you to write the handoff document, update this documeent (`docs/HANDOFF.md`) with fresh context

### Documentation Principles

- Use descriptive behavioral language:
  - Describe user actions step-by-step: "User clicks X → component emits Y → host does Z"
  - Show data and action flows with arrows such as `ComponentA → ComponentB`

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
