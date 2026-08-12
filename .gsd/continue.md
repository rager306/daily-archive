# Handoff — 2026-08-12 (M387 optimizer-ready pipeline)

## M386-57ae9l still active

Rust-only D237 acquisition delivered `9d57d53e`. Remaining typed article.v00.02, bounded network retry/backoff + partial/checksum conflicts, then Python downloader archive and Rust-only acceptance. D127 false.

## M387 delivered

- S01 `ccb27e08`: 14-stage DATA ledger/runtime audit.
- S02 `aebb7bce`: ACAP context conservation; 30 parse/extract profile, 140 candidates/117 grounded, 12 no_claim.
- S03 `6bd13514` + `d9fbd418`: dual cache lineage, whole-pipeline metrics, real 1/10/30 cache profile and 200-paper replay; UAT PASS.

## M387/S04 active

Foundation `3c2162ca`: ADR-059/D238 PromptExecution/FSM, optimizer disabled.
Summary authority `02761fe0`: summary PromptExecution + candidate-only graph writes.
9router correction `7b27ec55`: models.yaml canonical; summary uses paper-summary-generate-default; typed registry/OpenAI client/live 592-model inventory.
Figure migration `153691c4`: typed-media PromptExecution/9router, figure-qa-primary binding, no direct-provider fallback, lineage-gated candidates, cache identity fixed.

### Frozen offline evaluation loop delivered `667de01e`

- Added application evaluator with source-digest fixtures, required/forbidden fact oracles, train/validation partitions and deterministic metrics: schema validity, grounding, faithfulness, coverage and paraphrase stability plus latency/token/cost observations.
- Enforces nonzero same-n, contamination separation, validation gap, hard-invariant, latency and cost budgets through PromptEvaluation FSM contracts.
- Added `data/prompt_eval/{fixtures,policy}.yaml`, `prompt_eval` CLI and durable `artifacts/runtime/prompt_eval_m387.json`.
- Both article_summary and figure_analysis run through FSM state evaluated with baseline_n=candidate_n=2 and executable perfect synthetic contract metrics. Candidate remains eligible=false, deployed=false, import_eligible=false.
- Added readiness projection ingestion: `prompt_optimizer_readiness_m387.json` now reports evaluated/same-n=2 for both tasks, review_status=synthetic-contract-fixture, production_acceptance_eligible=false, optimizer=false, auto_promote=false, D127=false.
- No model inference was requested. This proves evaluator mechanics only, not production quality.

S04-F03 remains blocking: current fixtures are synthetic contract fixtures, explicitly not production acceptance. Commit 667de01e intentionally does not close S04 or enable GEPA/DSPy.

Verification for latest wave: prompt_evaluation unit/negative tests PASS, both CLIs compile/run, deterministic report digests validated, per-crate Clippy PASS, full architecture PASS, Governor 36 PASS/0 violations, GitNexus reindexed. Structured logs: prompt_eval/readiness task_count=2, no WARN/ERROR, optimizer/auto_promote/D127 false.

### Exact next action S04/F03

1. Promote real source-bound reviewed fixtures, not synthetic assertions:
   - summary fixtures from canonical article/revision text with independently reviewed facts;
   - figure fixtures from `2507.19457/Figure3.context.json`, its exact image SHA and independently reviewed evidence facts.
2. Preserve contamination-separated train/validation sources; record reviewer/evidence refs and fixture approval artifact.
3. Re-run prompt_eval/readiness. Keep eligible/deployed false even after review; acceptance only proves evaluation corpus quality.
4. Runtime UAT/full gate and close S04. Then S05 stage profiles/ablations, S06 Samyama active path, S07 RuVector/SONA retrieval quality.

## LLM audit current

- Summary and figure: both PromptExecutionPort + models.yaml/9router, candidate-only.
- Not generative: RuleBasedExtractor, GROBID, extractive summary, FD_API embedding, MCP prompt templates.
- GEPA/DSPy may later propose candidates but cannot deploy/promote/import.
- RuVector/SONA are retrieval/graph optimization surfaces, not prompt optimizers; require recall/MRR/nDCG/filter/latency and active-path evidence.

## Session findings

- Codebase Memory MCP timed out; GitNexus authoritative and reindexed.
- Real Figure3 context/analysis assets exist and are the best reviewed-fixture seed, but existing analysis is historical provider output and cannot be used as its own oracle without independent review.
- No raw provider body/secret persisted; no paid inference issued.
- GSD journal no S04 stuck-loop. Generated trajectory report remains unstaged.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D239 models.yaml+9router. D136 DSPy disabled. GEPA no auto-promote/dual F1+same-n+val_gap. Samyama sole store. Cache only pure compute; materialization always runs. RuVector/SONA/RocksDB only after active-path evidence.
