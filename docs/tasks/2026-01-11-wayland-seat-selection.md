# Wayland seat selection (`--seat`)

## Goal

Support multi-seat setups (sway/wlroots) by allowing playback to attach the virtual keyboard to a specific Wayland seat.

Done means:

- `drafter play` / `drafter run` accept `--seat <NAME>` (e.g. `seat0`, `seat1`).
- Playback binds the matching `wl_seat` and creates the virtual keyboard for that seat.
- If the requested seat is missing, the error lists discovered seat names.

## Current state

- Playback bound the first `wl_seat` global (effectively always using the first seat, typically `seat0`).

## Approach

- Enumerate `wl_seat` globals from the registry.
- When `--seat` is provided, bind all seats, run a roundtrip to receive `wl_seat.name` events, then select by name.
- Create `zwp_virtual_keyboard_v1` using the selected seat.

## Decisions

- Seat identifier: `wl_seat.name` string.
- `--seat` is optional; default behavior remains “first seat”.
- No `--list-seats` flag; missing seat errors include the discovered seat names.

## Progress

- 2026-01-11: Added `--seat` to CLI (`play`/`run`) and threaded into playback.
- 2026-01-11: Playback enumerates seats and selects by `wl_seat.name` when requested.
- 2026-01-11: Updated `README.md` and `docs/ARCHITECTURE.md`.

## Notes / pitfalls

- Keyboard focus is per-seat; the target editor must be focused for that seat before the countdown ends.
