# Handoff — 2026-08-12 (M387 optimizer-ready pipeline)

## M386-57ae9l still active

Rust-only D237 acquisition delivered `9d57d53e`. Remaining typed article.v00.02, bounded network retry/backoff + partial/checksum conflicts, then Python downloader archive and Rust-only acceptance. D127 false.

## M387-jkpnfh delivered

- S01 `ccb27e08`: 14-stage DATA ledger/runtime audit.
- S02 `aebb7bce`: ACAP context conservation; real 30 parse/extract profile, 140 candidates/117 grounded, 12 no_claim.
- S03 `6bd13514` + `d9fbd418`: canonical cache key SHA, shared memory/filesystem contract, dual key+payload lineage, 14-stage optimizer-ready metrics, structured logging, real 1/10/30 cache profile and production 200-paper physical replay.

S03 UAT PASS:
- `9b0f6c56-ad1d-4828-809a-89423b3b7d75`: cold/hit/config-drift exact for 3/30/90 manifests; corruption visible; 200-paper replay applied=0 rejected=0 unobserved=0 unchanged=168239; D127 false.
- `4e6531cf-66ee-4d45-8bb5-048515753b52`: 17 focused cache/lineage/metrics tests.
- full architecture PASS, Governor drift_risk/0 violations, GitNexus reindexed.

## M387/S04 active, foundation delivered `3c2162ca`

Accepted ADR-059 + D238. Added:
- domain `PromptTaskSpec`, `PromptExecutionLineage`, `PromptEvaluation`, `PromptCandidate` FSM;
- FSM baseline_registered→candidate_proposed→evaluated→eligible|rejected; no deployed transition, no auto-promote, D127 false;
- same-n, validation-gap, contamination, hard-invariant, latency/cost gates;
- port `PromptExecutionPort` request/response;
- strict DATA `data/ontology/prompt_tasks.yaml` for `article_summary` and `figure_analysis`;
- model route selection remains `.env`: `ANTHROPIC_MODEL`, registry path `DA_PROMPT_TASK_REGISTRY`;
- optimizer false at registry+task level; GEPA/DSPy candidate-only and disabled;
- ADR-059 indexed; 5 focused tests, Clippy, full architecture PASS; Governor 0 violations; GitNexus reindex complete; Codebase Memory MCP timeout.

### Exact next action S04/T03

1. Implement adapter `PromptExecutionPort` wrappers over existing `LlmClient` and `MultimodalLlmPort` with strict template rendering, `.env` model resolution, prompt/schema/route/input/output hashes, tokens/latency/cost missingness and structured failure logs.
2. TDD migrate `SummaryUseCase` first: inject `PromptTaskSpec`/PromptExecutionPort, remove inline prompt and `model:""`, require valid candidate lineage before Summary/Entity graph writes, preserve extractive summary path.
3. Migrate FigureAnalysisUseCase using existing safe prompt/schema/model/cache lineage pattern; preserve authoritative=false/D127.
4. Offline frozen prompt eval report, candidate only, same-n+val_gap; then full UAT and close S04.

## LLM audit

Active generative paths only:
- `SummaryUseCase::generate_summaries` / `da summarize`: current HIGH gap until T03 migration (inline prompt, implicit adapter model, no full lineage/eval, graph writes).
- `FigureAnalysisUseCase`: candidate-only multimodal; already has prompt/schema/route/model/tokens/latency/cache lineage and is reference pattern.

Not generative: RuleBasedExtractor, GROBID, extractive summary, FD_API embedding, MCP prompt templates. RuVector is retrieval/backend optimization, not prompt optimization; remains S07 recall/MRR/nDCG/filter/latency gated.

## Session checks and Governor proposal

- Application cache profile logs contained no WARN/ERROR; completion event rows and D127 verified.
- GSD journal S03/S04: no stuck-loop/timeout entries.
- Governor active: drift_risk, 0 violations; only large `.git` + missing CHANGELOG unreleased heading flags remain.
- Future S08 ratchet: active generative call sites must reference PromptExecutionPort and a registered task; block inline provider model defaults, `optimizer_enabled=true`, deployed prompt candidates, or summary graph writes without lineage. Add only after product negative fixtures in S04.
- Generated `artifacts/project-trajectory/trajectory-report.json` remains unstaged.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D136 DSPy disabled. GEPA no auto-promote/dual F1+same-n+val_gap. Samyama sole store. Cache only pure compute; graph materialization always runs. No backend inference from stage ID. RuVector/RocksDB only after active-path evidence.
