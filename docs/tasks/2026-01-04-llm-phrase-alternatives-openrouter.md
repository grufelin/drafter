# Task: OpenRouter phrase alternatives (type then fix back)

Status: completed

## Goal

Support “type a slightly different phrase, then later edit it back so the final text matches exactly”, optionally using OpenRouter-based suggestions.

## Implementation summary

- LLM output format: `Vec<PhraseAlternative { original, alternative }>` validated per paragraph.
- Planner API: `drafter::planner::generate_plan_with_phrase_alternatives(final_text, cfg, alternatives_by_paragraph, rng) -> Result<Plan>`.
- Paragraph splitting: non-empty paragraphs split on blank lines (`\n\n`), skipping leading/trailing newline-only regions.
- Planning behavior: when the cursor reaches an `original` span, type `alternative` and enqueue an outstanding error to later replace `alternative` back to `original`.
- Phrase-level fixes are restricted to sentence/paragraph boundaries during the forward typing pass, plus the near-end review pass.

## CLI integration

`drafter plan` and `drafter run` support:

- `--llm` (off by default)
- `--llm-model`
- `--llm-max-suggestions`
- `--llm-rewrite-strength` (`subtle|moderate|dramatic`)
- `--llm-max-concurrency` (`1-10`)
- `--llm-cache` (JSON; contains your draft text)
- `--llm-on-error` (`fallback|error`)

Remote fetching uses `drafter::llm::openrouter::OpenRouterParagraphRephraseClient` behind `--features llm`.
If `--features llm` is not enabled, `--llm` only works when `--llm-cache` points to an existing cache file.

## Reproducibility / caching

- `--llm-cache` saves/loads the exact JSON used to generate phrase alternatives.
- Pair with `--seed` to make the full flow deterministic (plan randomness + phrase suggestions).

## Tests

- `tests/llm_validation.rs` covers `validate_phrase_alternatives()`.
- `tests/planner_phrase_alternatives.rs` asserts the plan both types an alternative and produces the exact final text after simulation.
- `tests/openrouter_live_smoke.rs` is an ignored live-network smoke test.

## Follow-ups (optional)

- Add model reasoning configuration and verify behavior across multiple models.
- If OpenRouter structured outputs become unreliable, add a raw JSON fallback mode.

## Build/test

```bash
cargo test
cargo test --features llm
```
