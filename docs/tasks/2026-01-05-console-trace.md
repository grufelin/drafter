# Console typing trace (playback)

## Goal

Add lightweight console logging during playback so users can see what `drafter` is trying to do without printing every key event.

- When normal typing is interrupted by editing/navigation, log: `Typing "<text>"...`
- When a correction replaces earlier text, log: `Replace "<wrong>" with "<correct>"...`
- Newlines must be rendered as `\n` in logs.
- Logging is **on by default** and can be disabled via a CLI option.

## Current state

- Playback (`src/playback.rs`) replays low-level `Action`s and prints only coarse status (countdown, abort).
- Planner precomputes everything; playback does not know the draft text explicitly.

## Approach

- Add a small playback tracer that observes the `Action` stream during `play_plan()`.
- Decode pressed key events into characters using the existing US-QWERTY mapping logic (similar to `sim::simulate_typed_text`).
- Maintain a tiny internal editor model (buffer + cursor) so backspaces can capture the exact deleted span.
- Precompute trace events and print them before the associated typing/correction sequence starts.

## Decisions

- Print to stderr (`eprintln!`) to match existing status output and avoid interfering with JSON output.
- Do not truncate text (per request); provide `--no-trace` for sensitive drafts.

## Progress

- 2026-01-05: Started.
- 2026-01-05: Added `src/trace.rs` (action→text decoder + tiny editor sim) and wired it into playback.
- 2026-01-05: Added `--no-trace` to `play`/`run` (trace on by default) and documented it in `README.md`.
- 2026-01-05: Added deterministic unit tests for the tracer (`tests/trace_console.rs`).
- 2026-01-05: Changed trace output to pre-announce sequences (schedule events by action index).
- 2026-01-05: Added ANSI color on trace verbs only ("Typing" blue, "Replace" yellow).
- 2026-01-05: Updated `docs/ARCHITECTURE.md` to mention `src/trace.rs` and the playback trace.

## Notes / observations

- This feature necessarily prints portions of the draft text to the console; users should disable tracing for sensitive input.
- Replacement completion is detected by the next edit/navigation key (delayed fixes) or by the first non-word character typed after an end-of-buffer correction.
- Initial implementation printed trace lines after the fact; switched to precomputing a schedule so logs print before actions.
- ANSI colors are always emitted; if this is annoying when piping output, consider adding tty/`NO_COLOR` detection later.
