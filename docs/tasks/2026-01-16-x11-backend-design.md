# 2026-01-16 — X11 playback backend design

## Context

`drafter` splits work into two phases:

- Plan: precompute an offline `Plan` (actions + keymap config)
- Play: replay the plan into the currently focused editor

Recent work landed:

- `--backend <auto|wayland|x11>` exists on `play`/`run` (default `auto`).
- `play`/`run` fail fast on unsupported environments/backends; `plan` intentionally skips backend checks.
- Playback now has a backend seam:
  - Selection/dispatch: `src/playback/mod.rs`
  - Wayland implementation: `src/playback/backends/wayland.rs`
  - X11 implementation: `src/playback/backends/x11.rs` (feature-gated; enabled by default).

## Non-negotiable constraints (safety)

- Keyboard events only (no mouse, no clipboard).
- Never read editor contents.
- Planner stays fully offline/inspectable; playback only replays.
- Treat drafts as sensitive; avoid logging draft text.

## Goal

Design and implement an X11 playback backend that mirrors the Wayland backend’s shape and safety, while accepting that X11 cannot receive a per-client keymap.

This document covers the research and decisions for the *first viable* backend + a roadmap for hardening.

## Recommendation (first viable backend)

### Injection mechanism: XTEST

Use the XTEST extension (XTestFakeKeyEvent / `FakeInput`) to inject `KeyPress`/`KeyRelease` events.

Why:

- Closest to “real input” injection on X11; generally works with toolkits/browsers.
- Routes to the server’s current input focus (closest analogue to Wayland “focused surface”).
- Simpler and more reliable than `XSendEvent` for keyboard typing.

What we will do:

- Connect to the X server via `DISPLAY`.
- `QueryExtension("XTEST")` to ensure XTEST exists; error cleanly if missing.
- Use `xtest_fake_input` to emit `KeyPress`/`KeyRelease` only.

What we will *not* do:

- No `XSendEvent`-based key event delivery.
- No XInput2 multi-device injection for MVP.
- No mouse/button/motion injection.

### Focus targeting

- With XTEST, injected keyboard events follow the X server’s focus policy.
- UX relies on the existing countdown prompt: user focuses the editor window before playback.

Optional MVP validation:

- Query input focus once right before playback starts and error if focus looks invalid.
- This uses only window IDs (no reading window contents).

### Keycode strategy: require US layout

Current plans are expressed in Linux evdev keycodes and assume a US-QWERTY mapping (`src/keyboard.rs`, `src/keymap.rs`).

On Wayland, `drafter` can push a US XKB keymap into the virtual keyboard. On X11, there is no equivalent “set keymap for this client/device”, so we must instead enforce that the *X server* is using a compatible layout.

MVP approach:

- Map evdev keycodes → X11 keycodes via `x11_keycode = evdev_keycode + 8`.
- Validate the server keymap looks like US-QWERTY by checking several representative keys’ keysyms via `GetKeyboardMapping`.
- If mismatch, fail early with an actionable error (e.g. suggest `setxkbmap us`).

We will not implement complex translation (keysym-level mapping) in the first backend.

**User approval:** Only support US layout; no complex translation.

### Handling `Action::Modifiers`

- On Wayland we send `zwp_virtual_keyboard_v1.modifiers()`.
- On X11 there is no equivalent “set depressed modifiers state” for our injected stream.

MVP decision:

- Treat `Action::Modifiers {..}` as a no-op on X11.
- Rely on modifier key press/release events already present in the plan (Shift/Ctrl).

### Dependency choice and feature gating

Use `x11rb` as the Rust X11 implementation:

- Safer API than raw Xlib FFI.
- Supports XTEST requests via `x11rb::protocol::xtest::ConnectionExt` (e.g. `xtest_fake_input`).

Keep dependencies optional:

- Add a crate feature such as `x11 = ["dep:x11rb", "dep:x11rb-protocol"]` (depending on what’s required).
- Default build may include X11 (depending on project defaults); support `--no-default-features --features wayland` to build Wayland-only.

### CLI/UX decisions

- `--backend x11` remains the entry point.
- Errors should mirror `src/playback/mod.rs` fail-fast style.

`--seat` interaction:

- X11 backend will hard-error if `--seat` is provided.

**User approval:** hard error on passing `--seat` on X11.

## Clean failure conditions and messages

Fail fast with clear guidance:

- No `DISPLAY` / cannot connect: “failed to connect to X11; is DISPLAY set?”
- Missing XTEST extension: “X11 backend requires XTEST extension”
- Keymap not US: “X11 backend currently requires US layout; try `setxkbmap us`”

Never include draft text in logs/errors.

## Roadmap (hardening)

1. MVP backend (above): XTEST + `evdev+8` + US keymap validation.
2. Improve keymap detection:
   - more extensive keysym checks
   - optionally query XKB extension data (still verify via keysyms)
3. Optional layout-independent typing:
   - translate plan intent (keysyms) to server keycode+level; accept reduced “plan is the truth” clarity
4. Optional multi-seat/multi-device:
   - investigate XI2; introduce X11-specific device selection flag rather than overloading Wayland `--seat` semantics

## Implementation plan (next steps)

- Add `src/playback/backends/x11.rs` behind a `x11` cargo feature.
- Extend `src/playback/backends/mod.rs` to include x11 when feature enabled.
- Update `src/playback/mod.rs`:
  - If backend resolved to X11 and feature disabled, error with “compile with --features x11”.
  - If enabled, call `play_plan_x11(plan, ...)`.
  - Hard error when `seat_name.is_some()` on X11.
- Add a small keymap validation helper in the X11 backend (keysym checks for representative keys).
- Ensure abort path releases Shift/Ctrl (best effort), matching Wayland backend cleanup.
