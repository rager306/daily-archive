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
9router correction `7b27ec55`: models.yaml canonical, summary via paper-summary-generate-default, typed registry/OpenAI client/live 592-model inventory.

### Figure PromptExecution migration delivered `153691c4`

- PromptExecutionPort now carries typed bounded `PromptMedia` with MIME, bytes and exact SHA; text adapter rejects media.
- Added OpenAI-compatible 9router multimodal adapter: exactly one supported image, data-URL request, strict prompt variable set, real JSON Schema validation, token/latency/error mapping and full task/route/model/media/input/output lineage.
- Added compatibility `PromptMultimodalBridge` so existing cache-aware FigureAnalysisStage remains reusable while product composition is PromptExecution-only.
- FigureAnalysisUseCase now rejects any model response without valid PromptExecution lineage and binds prompt task, route, model binding, input and output hashes in candidate provenance.
- Figure cache configuration now uses registry task/schema SHA, image SHA and resolved model binding rather than legacy prompt/schema constants.
- `analyze_figure` no longer guesses routes, probes `/models/image-to-text`, accepts inline model/URL flags or silently falls back to direct MiniMax. It resolves `figure-qa-primary` from models.yaml.
- Added real models.yaml binding `figure-qa-primary` → `novita/qwen/qwen3-vl-235b-a22b-instruct`, verified present in live 9router inventory. Two previously remembered Qwen IDs were absent and not used.
- Added durable `artifacts/runtime/prompt_optimizer_readiness_m387.json`: summary+figure exact prompt/route/schema/model identities and 13 lineage fields. Frozen baseline/candidate evaluation is still no_claim; FSM baseline_registered; eligible/deployed/optimizer/auto-promote/D127 false.
- Added Governor high rule preventing CLI generative paths from composing Anthropic provider clients/routes directly; 36 Governor tests PASS.
- Local ephemeral OpenAI-compatible multimodal roundtrip verifies actual image data URL and token lineage; media missing/checksum/type failures happen before network. No external inference was issued.

Verification for 153691c4: 19 focused domain/port/adapter/application tests, CLI all-target check, per-crate Clippy, full architecture PASS, Governor drift_risk/0 violations, GitNexus reindexed. GitNexus risk CRITICAL expected due media/cache/usecase/CLI boundary change. Structured readiness log has task_count=2, optimizer=false, auto_promote=false, D127=false and no WARN/ERROR.

### Exact next action S04/T04

1. Create frozen reviewed summary and figure fixtures with independent expected grounding/schema facts. Do not use the candidate implementation as oracle.
2. Implement `prompt_eval` harness for baseline/candidate same-n evaluation: schema validity, grounding, faithfulness, coverage, paraphrase stability, latency, tokens, estimated cost; contamination, val_gap, latency/cost budget negatives.
3. Run only candidate evaluation (no deploy, no graph authority); update readiness report from no_claim to evaluated only with durable evidence.
4. Full UAT and close S04. Then proceed to S05 stage profiles/ablations, S06 Samyama active path and S07 RuVector/SONA retrieval quality.

## LLM audit current

- Summary and figure: both PromptExecutionPort + models.yaml/9router, candidate-only.
- Not generative: RuleBasedExtractor, GROBID, extractive summary, FD_API embedding, MCP prompt templates.
- GEPA/DSPy may later propose candidates but cannot deploy/promote/import.
- RuVector/SONA are retrieval/graph optimization surfaces, not prompt optimizers; require recall/MRR/nDCG/filter/latency and active-path evidence.

## Session findings

- Codebase Memory MCP timed out; GitNexus authoritative and reindexed.
- Figure CLI contained a real hidden direct-provider fallback and stale model discovery route; both removed.
- Live 9router inventory was used to repair figure binding to an actually present visual model.
- No raw provider body/secret persisted; no paid inference was issued.
- GSD journal has no S04 stuck-loop events. Generated trajectory report remains unstaged.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D239 models.yaml+9router. D136 DSPy disabled. GEPA no auto-promote/dual F1+same-n+val_gap. Samyama sole store. Cache only pure compute; materialization always runs. RuVector/SONA/RocksDB only after active-path evidence.
