# Handoff — 2026-08-12 (M386 Rust migration + M387 semantic pipeline active)

## Active milestone M386-57ae9l

Goal: Rust-only production pipeline (D237). Python remains only governor/tests/migration analysis/frozen evidence.

Delivered commit `791f7c7b`:
- typed `batch-acquisition-report.v1`;
- `BatchAcquisitionUseCase` over `SourceAcquisitionPort`;
- production `da acquire-arxiv` command (IDs/file, progress, pacing, atomic report, D127 false);
- README and GEPA reproduction use Rust command;
- boundary audit `artifacts/runtime/rust_pipeline_boundary_m386.json`;
- real M385 no-network replay: 580 total, 579 catalog hits, one withdrawn failure `2507.23261`, zero Python subprocesses.

Remaining M386:
- S03 centralize article.v00.02 metadata in typed Rust builder and delete Python duplicate;
- S04 migrate/archive remaining active product script;
- S05 evidence-backed governor guardrail;
- S06 Rust-only end-to-end acceptance.

One product Python script remains: `scripts/download_arxiv_to_catalog.py`; do not delete until Rust network/metadata parity and M385 checksum replay pass.

## Active milestone M387-jkpnfh

Goal: deep stage semantics/context/cache/scheduler/profile/SMP/Samyama/ruvector optimization.

### S01 delivered `ccb27e08`

Canonical stage ledger:
- DATA: `data/ontology/pipeline_stage_contracts.yaml`;
- 14 stages: acquisition, catalog, parse, revision materialization, extraction compute/materialization, embedding, summary, readiness, cluster, resolution, assurance, checkpoint, query;
- each stage names owner, semantic input/output, context additions/losses, deps, hard invariants, consumers, cache mode, failure class, conditionality and graph truth boundary;
- domain rejects duplicate IDs, missing deps, cycles, incomplete semantics, graph/cache mismatch and D127;
- strict YAML loader rejects unknown fields/oversize;
- application runtime audit checks observed order, context digests/counts and failure visibility;
- durable report `artifacts/runtime/pipeline_stage_ledger_m387.json`: 14 stages, zero findings, D127 false;
- UAT `6a431853-f9bf-4cd0-9e6a-5d71ea5eabe4`;
- full architecture PASS; GitNexus reindexed.

### Exact next action: M387/S02

Implement context conservation:
1. typed context metrics contract (bytes/text/sections/tables/figures/citations/spans/VID grounding/duplicates/navigation/entities/evidence/summaries/readiness/missingness);
2. pure observers for ParsedArticle, RevisionContentContext, ExtractedEntity, materialization, summaries/readiness;
3. strict DATA policy `context_conservation_policy.yaml` for quality thresholds (not hard invariants);
4. exact-v1 and 30-paper profiles correlated with graph tasks/readiness;
5. expose context gained/lost/duplicated/missing per stage.

Then M387/S03 cache lineage, S04 bounded DAG scheduler/SMP, S05 profiles/ablations, S06 Samyama active-path profile, S07 ruvector applicability/retrieval quality, S08 acceptance/governor.

## Deep audit conclusions

- Canonical Rust flow: acquisition→catalog→parse(cacheable)→revision materialization(always graph)→extraction compute(cacheable)→extraction materialization(always graph)→conditional embedding/summary/readiness/cluster/resolution→assurance→checkpoint→query.
- Graph materialization must never be skipped by compute cache.
- Current scheduling is stage-serial; only gold prepare uses scoped workers and unbounded mpsc, then deterministic single writer.
- No cooperative cancellation or bounded backpressure in general pipeline yet.
- M384 current 200-paper graph materialization: 18.59s, 229MB, strict semantic equal, replay applied=0, D127 false.
- Samyama is active in-memory HOT path; RocksDB inactive; global writer RwLock remains but measured throughput accepted.
- ruvector active only in da-mcp RRF hybrid and feature-gated PPR. `RuVectorBridge` is stub. Do not tune/promote ruvector before S07 quality/latency benchmark.
- Missing ADR-051 priority: shared PipelineCache contract, ParserPort contract, acquisition wiremock/mixed failures, Samyama revision/extraction replay outcomes, scheduler race/fairness, vector recall/ties/filters/no-NaN/temporal/provenance.
- Codebase Memory MCP timed out; GitNexus and direct code are authoritative.

## Locks

D127 false. D237 Rust-only product pipeline. No Rust→Python fallback. Samyama sole store. No implicit revision default. No legacy Paper summaries. Cache only pure compute; graph truth always materialized. Vendor trees require clean isolation before edits. RocksDB/ruvector tuning only after active-path evidence. DSPy disabled.
