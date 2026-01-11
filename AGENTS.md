# AGENTS.md

## Building



## Testing

- Prefer testing planner invariants and simulations (`tests/`) rather than trying to test playback in CI.
- Keep RNG deterministic when diagnosing planner behavior by using `--seed` on `plan`/`run`.
