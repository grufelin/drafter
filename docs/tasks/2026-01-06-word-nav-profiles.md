# Word navigation profiles (chrome vs compatible)

## Goal
- Add `--profile <chrome|compatible>` for `drafter plan` and `drafter run`.
- Preserve existing behavior as profile `chrome` and make it the default.
- Add profile `compatible` that only emits Ctrl+Left/Right when the predicted jump is “safe” across editors; otherwise use plain Left/Right.

## Findings
- `output.txt` shows cross-editor drift for Ctrl+Left/Right around punctuation blocks (quotes, apostrophes, dashes, ellipses, brackets, separators) and across newlines.
- Planner correctness depends on the planner’s internal cursor model matching the real editor caret; drift causes later corrections to apply at the wrong location.
- Observed on KDE: Ctrl+Arrow can still drift near hyphenated words even when the traversed span is only ASCII alphanumerics (toolkits disagree about stops adjacent to punctuation, e.g. `mid-sentence`).

## Approach
- Introduce a `WordNavProfile` enum (`Chrome`, `Compatible`) and thread it through `PlannerConfig`.
- Keep `chrome` behavior unchanged.
- For `compatible`, gate emission of Ctrl+Left/Right using a conservative predicate:
  - Only allow Ctrl+Arrow when the traversed characters are exclusively ASCII alphanumerics and ASCII spaces.
  - Additionally require the characters immediately adjacent to the jump endpoints are also in that safe set (prevents “alnum-only span but punctuation-adjacent stop” drift).

## Progress
- Added `WordNavProfile` + `compatible_ctrl_span_is_safe` + `compatible_ctrl_jump_is_safe` in `src/word_nav_profile.rs`.
- Threaded profile through `PlannerConfig` and correction navigation in `src/planner.rs` (default `chrome` preserved).
- Added `--profile <chrome|compatible>` to `drafter plan` and `drafter run` in `src/main.rs`.
- Added tests in `tests/word_nav_profiles.rs` (span + jump safety).
- Updated docs: `docs/ARCHITECTURE.md`, `README.md`.
- Added `docs/troubleshooting-word-navigation.md`.
- Ran `cargo fmt --check` and `cargo test` (pass).

## Next steps / open questions
- Re-run the KDE reproduction with the latest binary (expect fewer/no Ctrl+Arrow near punctuation/hyphens).
- Optional: add a higher-level test asserting `compatible` emits fewer Ctrl+Arrow events around risky punctuation.
