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

## M387/S04 active — current blocker

### What works

- Offline evaluator: source-digest fixtures, required/forbidden facts, train/validation contamination, budget gates, same-n metrics, FSM `evaluated`.
- Source-bound fixtures: independently reviewed, SHA-bound, fail-closed (self-oracle, path escape, digest drift, unbound excerpt).
- Adapter hardening: `stream:false`, empty-body detection, `<think>` handling, JSON extraction, `finish_reason` logging, `enable_thinking:false`, `response_format:json_object`.
- Bounded inference runner: `data/prompt_eval/inference.yaml` strict policy (DATA-driven), model binding `minimax/MiniMax-M3`, durable attempt/failure ledger, no retry/fallback/promotion.
- Governor ratchets: prompt-eval DATA cannot claim production acceptance or enable optimization.

### What does not work (model/proxy incompatibility)

MiniMax-M3 through 9router does NOT produce structured JSON output:
- `response_format: {"type": "json_object"}` is silently ignored by the proxy.
- `enable_thinking: false` disables `<think>` reasoning but model still returns markdown prose.
- Model produces valid, grounded summaries in prose — but NOT in JSON format.
- `finish_reason: "stop"` confirms model chooses to stop (not truncated).
- Durable evidence: `artifacts/runtime/prompt_eval_inference_m387.json` records attempted=1, completed=0, error_class=provider/schema.

### Options for next session

1. **Try different model**: `agnes-ai/agnes-2.0-flash` is already bound and might follow JSON instructions through the same proxy.
2. **Fix proxy**: 9router needs to pass through `response_format` to the upstream model API.
3. **Prose-to-JSON converter**: Add a structured-extraction adapter that parses prose into JSON schema (fragile, not recommended).
4. **Accept current state**: Adapter hardening is production-ready; model/proxy JSON compatibility is an external dependency.

## LLM audit current

- Summary and figure: both PromptExecutionPort + models.yaml/9router, candidate-only.
- GEPA/DSPy disabled. D127 locked. D238/ADR-059 PromptExecutionPort/FSM.
- models.yaml owns provider/model/protocol; .env owns host/key/timeout overrides.
- No hardcoded config: max_calls, timeout, variants, task routes in inference.yaml.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D239 models.yaml+9router. D136 DSPy disabled. GEPA no auto-promote. Samyama sole store. Cache only pure compute. RuVector/SONA only after active-path evidence.
