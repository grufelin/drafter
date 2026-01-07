# AGENTS.md

## Purpose

`drafter` is a Linux/Wayland typing simulator: given a “final draft” plain-text input, it **plans** a full sequence of human-like keyboard events and then **plays** them into the currently focused editor surface.

## Where to start

- `docs/ARCHITECTURE.md` — architecture, module map, planner algorithm.
- `README.md` — build + CLI usage.
- `docs/typing-behavior-requirements.md` — behavioral + safe-key requirements.

## Workflow
### Documentation updates

After every round, when done with the editing:
	- If you change CLI commands/options, update `README.md`.
	- If you change planning behavior/capabilities, algorithms, or core architecture, update `docs/ARCHITECTURE.md`.
	- If you change safe-key policy or constraints, update `docs/typing-behavior-requirements.md`.
	- Update the task log file as below

### Task logs

For major jobs, keep a short working note in `docs/tasks/` so handoffs are easy and future sessions have context without bloating stable docs.

- Create one file per job: `docs/tasks/YYYY-MM-DD-<short-slug>.md`
- Keep it updated as you work:
  - Goal (what “done” means)
  - Current state (relevant constraints/behaviors)
  - Approach (what you plan to change)
  - Decisions (tradeoffs, why)
  - Progress (what changed; key files)
  - Next steps / handoff (what’s left, pitfalls)

## Non-negotiable constraints

- Keyboard events only; never read editor contents.
- No clipboard operations (no copy/cut/paste).
- Only emit keys/shortcuts in the allowlist in `docs/typing-behavior-requirements.md`.
- Planning must fully precompute the full action sequence before playback.

Treat input drafts as potentially sensitive:

- Avoid logging draft text.
- Do not add code that reads the focused window text, reads the clipboard, captures the screen, or records keystrokes.

## Testing expectations

- Prefer planner-focused, deterministic tests (seeded RNG) in `tests/`.
- Playback is hard to test in CI; test planner invariants instead.