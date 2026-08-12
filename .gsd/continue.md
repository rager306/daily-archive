# Handoff — 2026-08-12 (M387 optimizer-ready pipeline)

## M386-57ae9l still active

Rust-only D237 acquisition delivered `9d57d53e`. Remaining typed article.v00.02, bounded network retry/backoff + partial/checksum conflicts, then Python downloader archive and Rust-only acceptance. D127 false.

## M387 delivered

- S01 `ccb27e08`: 14-stage DATA ledger/runtime audit.
- S02 `aebb7bce`: ACAP context conservation; 30 parse/extract profile, 140 candidates/117 grounded, 12 no_claim.
- S03 `6bd13514` + `d9fbd418`: dual cache lineage, whole-pipeline metrics, real 1/10/30 cache profile and 200-paper replay; UAT PASS (`9b0f6c56...`, `4e6531cf...`).

## M387/S04 active

Foundation `3c2162ca`: ADR-059/D238, PromptTask/Lineage/Evaluation/Candidate FSM, PromptExecutionPort, strict prompt task DATA, optimizer disabled.

Production summary migration `02761fe0`:
- strict referential registry now owns real route policy + JSON Schema bytes; route/model/endpoint via DATA + `.env` (`ANTHROPIC_BASE_URL`, `ANTHROPIC_MODEL`, `DA_PROMPT_TASK_REGISTRY`);
- TextPromptExecutionAdapter enforces exact template variable set, nonrecursive rendering, provider identity, route temperature bounds, canonical variables+input digest, resolved prompt/schema/route/endpoint/model/output hashes, output JSON Schema, tokens/latency/effective bounds;
- bounded diagnostics log response chars+SHA+schema path, never raw provider payload; Governor information-preservation sink moved from removed raw summary path to adapter;
- `da summarize` uses PromptExecutionPort; inline prompt and `model:""` removed;
- Prompt lineage is validated before any graph mutation; unresolved section/provider/schema failures write zero graph state;
- LLM methods/datasets/metrics/quality are ignored as candidate fields, never canonical nodes; generative Summary nodes are `retrieval_eligible=false`, `authoritative=false`, `import_eligible=false` and carry full lineage;
- Summary schema DATA/hardcoded schema extended with lineage properties;
- real same-process Samyama ontology canary PASS: citation=2, independent deterministic quality=2, semantic tasks PASS, full replay stable, D127 false. Canonical artifact rebuilt.

S04-F01/F02 rework findings resolved with evidence. Verification: 10 focused tests, CLI all-target check, per-crate Clippy, full architecture PASS, Governor pytest 34 PASS, active Governor drift_risk/0 violations, GitNexus HIGH expected + reindex.

### Exact next action S04/T03 and T04

1. Migrate `FigureAnalysisUseCase` to PromptExecutionPort using its existing multimodal route/schema/model/token/cache lineage; preserve image bytes and candidate-only authority. Add multimodal adapter or extend request with typed media without weakening text adapter.
2. Ensure figure prompt task registry schema exactly matches `FigureAnalysisCandidate`; cache key includes task/schema/route/model identity; negative tests provider/route/media/schema.
3. Implement frozen offline prompt eval harness for summary+figure: independent fixtures, schema/grounding/faithfulness/coverage/paraphrase/latency/tokens/cost; same-n+val_gap+contamination/budget negatives; candidate reports only, optimizer=false.
4. Run S04 UAT/full gate, close slice. Only then add Governor ratchet blocking raw LLM call sites/inline provider defaults/summary graph writes without PromptExecution lineage.

## LLM audit current

- `SummaryUseCase`: migrated and safe candidate-only.
- `FigureAnalysisUseCase`: still on MultimodalLlmPort but already candidate-only with substantial lineage; migration pending.
- Not generative: RuleBasedExtractor, GROBID, extractive summary, FD_API embedding, MCP prompt templates.
- RuVector is retrieval backend, not prompt optimizer; S07 recall/MRR/nDCG/filter/latency gate remains.

## Session findings

- Reviewer caught two pre-commit blockers: ID hashes instead of resolved contract bytes, and summary LLM canonical entity authority. Both fixed before commit.
- Ontology canary originally relied on summary-generated QualitySignal facts; fixed by independent deterministic quality fixtures so the canary still proves quality endpoints without LLM authority.
- Combined Clippy command still reports vendor noise; per-crate Clippy and standalone architecture are authoritative. Full architecture final PASS.
- Codebase Memory MCP timed out again; GitNexus authoritative and reindexed.
- GSD journal had no S03/S04 stuck-loop entries. App logs had no raw provider payload; candidate flags false/false/false observed.
- Governor improvement applied: `raw_provider_payload_not_in_operational_logs` now guards `da-adapters/src/prompt_execution.rs`; 34 governor tests PASS. Future call-site ratchet waits for figure migration negative fixture.
- Generated `artifacts/project-trajectory/trajectory-report.json` remains unstaged.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D136 DSPy disabled. GEPA no auto-promote/dual F1+same-n+val_gap. Samyama sole store. Cache only pure compute; graph materialization always runs. No backend inference from stage ID. RuVector/RocksDB only after active-path evidence.
