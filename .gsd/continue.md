# Handoff — 2026-08-12 (M386 Rust migration + M387 optimizer-ready pipeline active)

## M386-57ae9l remains active

Rust-only production boundary D237. Delivered `9d57d53e` (`da acquire-arxiv`, typed report/use case, 580-ID replay 579 hit + withdrawn 2507.23261, D127 false). Remaining: article.v00.02 typed metadata, bounded network retry/backoff + partial/checksum conflicts, then Python downloader archive and end-to-end acceptance.

## M387-jkpnfh

### Delivered

- S01 `ccb27e08`: DATA-driven 14-stage acquisition→query ledger, strict DAG/cache/truth runtime audit.
- S02 `aebb7bce`: ACAP diagnostic context conservation; real 30 parse/extract profile, 140 candidates/117 grounded, 12 honest no_claim, no independent PASS; MEM752.
- S03 foundation `6bd13514`: canonical key SHA, shared memory/filesystem cache contract, dual key+payload lineage, whole-process metrics envelope and logging debt closure. S03 is NOT complete yet.

### Current S03 details

Added:
- `StageCacheKey::validate/key_sha256` in da-ports;
- `MemoryPipelineCache` and shared contract vs FilePipelineCache: cold/hit, config drift, retry expiry, manifest, concurrent same-key, visible corruption;
- `CacheLineageEvidence` + application audit: key SHA and payload SHA both mandatory; same bytes/different keys distinct; cache cannot claim graph materialization skipped;
- `pipeline-metrics.v1`: exact 14 stage rows and fields for runtime FSM, wall/CPU/RSS/I/O, queue/worker, cache, context, tokens/model latency/cost, backend applicability, failure, D127;
- durable `artifacts/runtime/pipeline_metrics_m387.json`: 14 rows, parse/extract observed, 12 typed no_claim, optimizer_admission_verdict=null, D127 false, deterministic SHA;
- rolling JSON logging initialized in pipeline ledger and both materialization profile roots. Event verified: `pipeline_metrics_profile_complete`, rows=14, observed=2, D127 false.

Verification: 17 focused tests, Clippy, full architecture PASS, Governor drift_risk/0 violations, GitNexus detect risk medium expected + reindex complete. Codebase Memory MCP timed out again.

### Exact next action S03

1. Add application end-to-end test proving two identical runs: compute executor called once, materialization fake called twice; prompt/config drift reruns only affected compute while materialization still runs.
2. Bind real Samyama revision/extraction replay evidence: applied=0, rejected=0, unobserved=0, unchanged>0.
3. Add `pipeline_cache_profile.rs` for 1/10/30 cold/hit/config-drift/corruption timing and structured events; bind key/output hashes and D127.
4. Update docs, run full gate, atomic commit, UAT, close S03.

### Planned S04 Prompt Execution FSM

User requested full-process metrics and future GEPA/RuVector/DSPy readiness. Audit found exactly two active generative paths:
- production text `SummaryUseCase::generate_summaries` (`da summarize`) — HIGH gap: inline prompt, no prompt/model lineage/cache/FSM/eval, temperature 0.3, graph writes;
- candidate-only multimodal `FigureAnalysisUseCase` — good existing prompt/schema/route/model/tokens/latency/cache lineage pattern.

Not generative LLM: RuleBasedExtractor, GROBID, extractive summary, FD_API embeddings, MCP prompt templates.

D238 recorded: all generative calls must use future provider-neutral `PromptExecutionPort` and candidate FSM. S04 tasks:
1. domain/port PromptSpec/Lineage/Metrics/Candidate FSM baseline_registered→candidate_proposed→evaluated→eligible/rejected; illegal/auto promotion forbidden;
2. strict DATA `prompt_tasks.yaml`: prompt versions/schema/model route policy/bounds/eval suite; runtime endpoints/secrets in .env; optimizer disabled;
3. wrap summary+figure; require valid lineage before summary graph write; emit complete metrics; no hidden fallback;
4. frozen independent prompt eval: schema/grounding/faithfulness/coverage/paraphrase/latency/tokens/cost, same-n+val_gap; candidate reports only.

GEPA/DSPy remain disabled/candidate-only; D136 and no-auto-promote stay binding. RuVector is retrieval, not prompt optimization, and remains S07 benchmark-gated.

## Operational findings

- Whole-pipeline optimizer readiness is bootstrap/partial (~4.5/10): stage taxonomy/cache types strong; 12/14 stages lack live context/performance observations; scheduler/retrieval quality missing.
- Never infer backend from stage ID. `backend=unobserved` until runtime evidence; S06/S07 establish Samyama/RuVector hot paths.
- No aggregate M384 timing copied into revision/extraction rows because it cannot be partitioned honestly.
- GSD journal has no M387 S03 stuck-loop/timeout entries.
- `artifacts/project-trajectory/trajectory-report.json` remains unstaged.

## Locks

D127 false. D237 Rust-only product. D238 PromptExecutionPort/FSM. D136 DSPy closed. GEPA candidate-only/no auto-promote/dual F1+same-n+val_gap. Samyama sole store. Cache only pure compute; graph truth always materialized. No implicit revision. Vendor edits isolated. RocksDB/RuVector tuning only after active-path evidence.
