# AGENTS.md (src/playback)

## Preflight + countdown

- Treat `--countdown` as “time to focus”, not “time to initialize”.
- Prefer failing fast before the countdown by doing backend/seat/platform checks up front:
  - Resolve backend selection and reject incompatible flags (e.g. `--seat` on X11) before planning/countdown.
  - Wayland: connect + registry init + seat discovery/selection before countdown so missing seats error immediately.
  - X11: connect + XTEST/keymap/focus checks before countdown so unsupported setups error immediately.

## CLI integration

- Keep flag validation in `playback` helpers (e.g. a preflight function) and call them from `src/main.rs` so `drafter run` fails before planning when playback flags are invalid.
