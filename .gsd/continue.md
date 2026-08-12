# Handoff — 2026-08-12 (M386+M387 complete)

## M386-57ae9l COMPLETE (7fcda94b)

Rust-only production pipeline (D237): all 6 slices delivered.
- Boundary audit classified 4 active Python scripts
- Bounded HTTP retry/backoff (DATA-driven, 5 unit + 4 wiremock contract tests)
- Article.v00.02 metadata builder (824 articles safety-locked)
- Python downloader archived with manifest
- Governor D237 guardrail (2 rules, 3 negative fixture tests)
- Pipeline acceptance verdict=accepted, 0 product Python, D127=false

## M387-jkpnfh COMPLETE

Semantic context pipeline optimizer-ready: PromptExecution FSM, frozen same-n
evaluation, measured hot path, retrieval quality, pipeline acceptance — all
candidate-only, D127 locked. 8 slices delivered and validated.

## Post-close fixes (current session)

- `f7a65ae8`: FSM transition invariant proptests (5 new, ADR-051 P1 state-machine model ✅)
- `8621ca2e`: serde roundtrip for PromptCandidate FSM types (ADR-051 §4.4)
- `9f8f4d1f`: **Bug fix** — multimodal adapter now has same JSON extraction + canonical
  output bytes as text adapter. This was the root cause of 2/4 figure schema failures.
  Re-running bounded figure inference should improve success rate.

## Next priorities

1. Re-run bounded figure inference with fixed multimodal adapter (should fix 2/4 schema failures).
2. Improve prompt candidate quality (raise summary coverage beyond 8461).
3. New milestone when user directs.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D239 models.yaml+9router. D136 DSPy disabled. GEPA no auto-promote. Samyama sole store. Cache only pure compute. RuVector/SONA only after active-path evidence.
