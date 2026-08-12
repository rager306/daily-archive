# Handoff — 2026-08-12 (M387 optimizer-ready pipeline)

## M386-57ae9l still active

Rust-only D237 acquisition delivered `9d57d53e`. Remaining typed article.v00.02, bounded network retry/backoff + partial/checksum conflicts, then Python downloader archive and Rust-only acceptance. D127 false.

## M387 delivered

- S01 `ccb27e08`: 14-stage DATA ledger/runtime audit.
- S02 `aebb7bce`: ACAP context conservation; 30 parse/extract profile.
- S03 `6bd13514` + `d9fbd418`: dual cache lineage, whole-pipeline metrics, 200-paper replay.
- Foundation `3c2162ca`: ADR-059/D238 PromptExecution/FSM.
- Summary `02761fe0`, 9router `7b27ec55`, figure `153691c4`.
- Frozen eval `667de01e`: offline FSM evaluator + synthetic fixtures.
- Source-bound fixtures `bda7f260`: independently reviewed GEPA abstract/section/Figure3/Figure5 SHA-bound evidence, fail-closed validation.
- Adapter hardening `8424e028`: reasoning-model compatibility, bounded inference runner.

## M387/S04 active — real same-n inference achieved (e74293a6)

### Progress

- **Real same-n inference works end-to-end**:
  - Summary: 4/4 executions via agnes-2.0-flash, valid JSON, schema-valid.
  - Figure: 2/4 executions via qwen3-vl, 2 schema quality-signals, full lineage.
  - Durable ledger: `artifacts/runtime/prompt_eval_inference_m387.json`.
- **Scoring works**: `prompt_eval_scored.rs` maps real outputs back into
  frozen source-bound fixtures, extracts canonical facts (structured + prose
  keyword evidence), produces honest metrics:
  - article_summary: grounding=1960, faithfulness=1960, coverage=7692,
    gap=4286, hard=false, eligible=false.
  - figure_analysis: grounding=10000, faithfulness=10000, coverage=4545,
    gap=10000, schema_failures=2/4, eligible=false.
- **FSM gates corrected**: `PromptEvaluation.validate` checks structural
  invariants only; `gates_passed()` holds execution gates and `mark_eligible`
  requires them. Gate failure yields `evaluated` with `eligible=false`.
- **Config data-driven**: per-task model bindings in `models.yaml`
  (prompt-eval-summary-agnes, prompt-eval-figure-qwen-vl), inference policy
  in `data/prompt_eval/inference.yaml`, latency/cost in `policy.yaml`.
- MiniMax-M3 remains unusable for JSON (prose-only through 9router);
  agnes-2.0-flash and qwen3-vl work with JSON extraction.

### Current blocker (honest status)

Neither task is eligible. Summary has low grounding/coverage vs oracle
(candidate prompt did not reproduce exact canonical facts); figure has schema
failures on 2/4 fixtures. Expected first-pass quality evidence, not a process
defect. GEPA/DSPy disabled; no auto-promotion; D127 locked.

### Next session options

1. Improve candidate prompt suffix in `data/prompt_eval/inference.yaml` and
   re-run bounded matrix to raise summary grounding/coverage.
2. Fix figure JSON emission (stronger JSON instruction or different model).
3. Accept current evidence and close S04 as "harness proven, quality
   ineligible" — honest fail-closed state.

## LLM audit current

- Summary and figure: both PromptExecutionPort + models.yaml/9router, candidate-only.
- GEPA/DSPy disabled. D127 locked. D238/ADR-059 PromptExecutionPort/FSM.
- models.yaml owns provider/model/protocol; .env owns host/key/timeout overrides.
- No hardcoded config: max_calls, timeout, variants, task routes in inference.yaml.

## Session findings

- Codebase Memory MCP timed out repeatedly; GitNexus authoritative and reindexed.
- MiniMax-M3 through 9router does not honor `response_format`; agnes-2.0-flash
  and qwen3-vl produce JSON after `stream:false` + `<think>` stripping + balanced
  brace extraction.
- Empty success responses, JSON preamble, code fences, and reasoning blocks are
  all handled by adapter hardening; durable failure ledgers record typed errors.
- No raw provider body/secret persisted; no paid inference beyond authorized
  bounded matrix (8 calls max, per-task same-n).
- GSD journal no S04 stuck-loop. Generated trajectory report remains unstaged.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D239 models.yaml+9router. D136 DSPy disabled. GEPA no auto-promote. Samyama sole store. Cache only pure compute. RuVector/SONA only after active-path evidence.
