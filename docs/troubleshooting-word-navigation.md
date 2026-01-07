# Troubleshooting: word navigation and Ctrl+Arrow drift

This document collects practical notes for debugging issues related to word navigation (Ctrl+Left/Right) during correction planning.

## Background

`drafter` must keep its internal cursor model aligned with the real editor caret. If they diverge, later correction sequences (move → backspace/delete → retype → move back) can hit the wrong location and corrupt text.

Word navigation is a common source of drift because different editors/toolkits interpret “word boundaries” differently, especially around punctuation.

To manage this, `drafter` supports word-navigation profiles:
- `chrome` (default): tuned to match Chrome/Docs-like Ctrl+Arrow semantics.
- `compatible`: conservative; only emits Ctrl+Left/Right when the predicted jump is highly likely to behave consistently across apps/toolkits; otherwise falls back to plain Left/Right.

## Symptoms of word-nav drift

Typical signs that Ctrl+Arrow semantics differ from the planner’s model:
- Corrections delete or insert text in the wrong place.
- Output contains duplicated or reordered fragments.
- A correction appears to “skip” over punctuation or stop unexpectedly.

When drift happens, subsequent corrections tend to compound the damage.

## Known risky boundaries

The following characters and boundaries are commonly implemented differently across editors/toolkits:
- Apostrophes in contractions/possessives: `'` and `’`.
- Hyphens and dashes: `-`, `—`, `–`.
- Quotes: `"`, `“”`, `‘’`.
- Ellipses: `...` and `…`.
- Punctuation and punctuation blocks: `. , ; : ? !` (including repeated runs).
- Brackets/parens/braces: `() [] {}`.
- Slash/backslash: `/` and `\\`.
- Underscore: `_`.
- Newlines: `\n`.

If a planned Ctrl+Arrow jump crosses (or stops adjacent to) any of these, different editors may land on different positions.

## How `compatible` decides when Ctrl+Arrow is allowed

`compatible` uses a conservative predicate to decide whether a *specific* Ctrl+Left/Right jump is safe:
- The span traversed by the jump must contain only ASCII alphanumerics and ASCII spaces: `[A-Za-z0-9 ]`.
- Additionally, the characters immediately adjacent to the jump endpoints must also be in that safe set.

This second rule matters because some editors disagree about stop positions next to punctuation (for example, hyphenated words can behave inconsistently even when the traversed span is all letters).

Implementation:
- Safety predicate: `src/word_nav_profile.rs` (`compatible_ctrl_jump_is_safe`).
- Emission gating: `src/planner.rs` in `navigate_left_to` / `navigate_right_to`.

## Bugs encountered and fixes

### KDE drift in `compatible` near hyphenated tokens

Observed:
- In some cases on KDE editors, `compatible` still drifted during corrections involving hyphenated words.
- The output showed corruption consistent with the caret being left in a different position than the planner expected.

Root cause:
- The original `compatible` check only validated the *traversed span* of a predicted Ctrl+Arrow jump.
- Some editors/toolkits can stop at different offsets when the destination is adjacent to punctuation (e.g. near `-`), even if the traversed characters are only ASCII alphanumerics.

Fix:
- Tightened `compatible` gating to also require the characters adjacent to the jump endpoints to be safe (`compatible_ctrl_jump_is_safe`).
- Added unit tests for this “alnum span but punctuation-adjacent stop” case.

## Debugging workflow (recommended)

1. Make runs reproducible:
   - Use `--seed <N>` so the same plan is generated.
   - Consider saving the plan with `--output plan.json` so you can replay/debug without regenerating.

2. Minimize variables:
   - If debugging word navigation, start with `--llm` disabled to reduce moving parts.
   - If you need LLM enabled, prefer `--llm-cache` so repeated runs don’t require network and are more stable.

3. Avoid leaking sensitive draft text:
   - Be aware that playback tracing can include typed text.
   - Use `--no-trace` when you don’t want console output to include draft content.

4. Probe Ctrl+Arrow behavior directly:
   - Use `src/bin/ctrl_nav_probe.rs` to generate probe plans that repeatedly press Ctrl+Left/Right and insert markers.
   - Compare landing behavior across Chrome/Firefox/KDE/GTK, and update the “risky boundaries” list if needed.

5. When in doubt, prefer determinism:
   - For robustness across editors, it’s better for `compatible` to emit more plain Left/Right than to risk drift.
