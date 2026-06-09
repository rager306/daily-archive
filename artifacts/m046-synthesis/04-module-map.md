# 04 — Module and Code Map

> **Inventory source:** `src/arxiv_archive/`, `scripts/`, `tests/`, `doc/contracts/m034-universal-kb/`
> **Synthesis layer:** 4 of 7
> **Stats:** 64 lib files, 64 verifiers, 154 tests, 5 contracts

## 0. Reading Order

This layer is the **ground truth of the codebase**, organized by the **8 bounded contexts** defined in `02-architecture-layers.md`. Each context has:

- Key `src/arxiv_archive/*.py` files
- Key `scripts/*.py` files
- Key `tests/test_*.py` files
- Related `doc/contracts/m034-universal-kb/*.md` documents
- ADRs and requirements that bind it

## 1. Repository Statistics

| Category | Count | Notes |
|---|---|---|
| Library files (`src/arxiv_archive/*.py`) | 64 | Python 3.x, stdlib-heavy (per D072) |
| Verifier scripts (`scripts/verify_*.py`) | 64 | one per major milestone + sub-verifiers |
| Test files (`tests/test_*.py`) | 154 | pytest-based, contract + integration |
| Contract documents (`doc/contracts/m034-universal-kb/`) | 5 | STATUS-MATRIX, CONTRACTS, SAFETY-INVARIANTS, FAILURE-TAXONOMY, ARTIFACT-DEPENDENCY-MODEL |
| ADRs (`doc/adr/m034/`) | 7 drafted + 1 planned (ADR-001) | Mermaid-assisted, LLM Reading Notes |
| Capability requirements (R001–R065) | 65 | 17 active, 48 validated |

## 2. Bounded Contexts → Module Inventory

### 2.1 Catalog & Intake

| Role | Files |
|---|---|
| Library | `data/article_catalog/` (managed externally) |
| Scripts | `scripts/select_m036_real_corpus_smoke_batch.py`, `scripts/select_m041_mixed_connectivity_batch.py`, `scripts/build_m031_catalog_backed_replay_selection.py`, `scripts/capture_m025_article_sources.py` |
| Tests | `tests/test_article_catalog_schema.py` |
| Verifiers | `scripts/verify_article_catalog.py`, `scripts/verify_m025_article_catalog.py` |
| Binding | R025, R031, R033, R035, R037, R064 |

### 2.2 Acquisition & Loader

| Role | Files |
|---|---|
| Library | `arxiv_archive/article_loader.py`, `arxiv_archive/arxiv_client.py`, `arxiv_archive/pdf_downloader.py`, `arxiv_archive/semantic_scholar.py` |
| Scripts | `scripts/replay_m025_article_loader.py`, `scripts/replay_m031_catalog_backed_acquisition.py`, `scripts/replay_m031_catalog_backed_loader_evidence.py`, `scripts/capture_m027_mixed_source_sources.py`, `scripts/capture_m029_unified_sources.py`, `scripts/build_m028_pdf_acquisition_diagnostics.py` |
| Verifiers | `scripts/verify_m027_source_acquisition_boundary.py`, `scripts/verify_m028_pdf_acquisition_diagnostics.py`, `scripts/verify_m029_unified_source_acquisition.py` |
| Binding | R014, R025, R030, R040, R050 |

### 2.3 Conversion & Parsing

| Role | Files |
|---|---|
| Library | `arxiv_archive/md_converter.py`, `arxiv_archive/page_index.py`, `arxiv_archive/full_text.py`, `arxiv_archive/structure_aware_chunking.py` |
| Scripts | `scripts/convert_m027_source_quality_boundary.py`, `scripts/convert_m029_unified_source_quality_boundary.py`, `scripts/replay_m031_parser_conversion.py` |
| Verifiers | `scripts/verify_m027_conversion_quality_boundary.py`, `scripts/verify_m029_unified_conversion_quality_boundary.py`, `scripts/verify_m031_parser_conversion_replay.py` |
| Binding | R014, R015, R027, R050 |

### 2.4 Chunking & Evidence

| Role | Files |
|---|---|
| Library | `arxiv_archive/chunk_import_contract.py`, `arxiv_archive/chunk_baseline_measurement.py`, `arxiv_archive/chunking_benchmark.py`, `arxiv_archive/structure_aware_chunking.py`, `arxiv_archive/evidence.py`, `arxiv_archive/bounded_chunk_repair.py`, `arxiv_archive/chunk_repair_contract.py`, `arxiv_archive/graph_readiness_manifest.py` |
| Scripts | `scripts/replay_m031_chunk_evidence.py`, `scripts/render_bounded_repair_prototype.py`, `scripts/render_chunk_repair_contract.py` |
| Verifiers | `scripts/verify_m022_final_gate.py`, `scripts/verify_m025_evidence_boundaries.py`, `scripts/verify_m031_chunk_evidence_replay.py`, `scripts/verify_bounded_repair_prototype.py` |
| Binding | R016, R027, R029, R036 |

### 2.5 Sidecar Probes

| Role | Files |
|---|---|
| Library | `arxiv_archive/universal_kb_sidecar_boundary.py` (Adaptix anti-corruption boundary) |
| Scripts | `scripts/probe_m033_opendataloader_adaptix_adapter.py`, `scripts/probe_m043_sidecar_runtime_readiness.py`, `scripts/build_m043_sidecar_packets.py`, `scripts/run_m044_live_grobid_candidate_probe.py` |
| Verifiers | `scripts/verify_m033_grobid_probe.py`, `scripts/verify_m033_opendataloader_adaptix_adapter.py`, `scripts/verify_m033_quantmind_pattern_study.py`, `scripts/verify_m033_combined_parser_architecture.py`, `scripts/verify_m033_external_parser_quality_plan.py`, `scripts/verify_m043_sidecar_runtime_readiness.py` (via M043 tests), `scripts/verify_m044_sidecar_architecture_guardrail.py` |
| Tests | `tests/test_m033_opendataloader_adaptix_adapter.py`, `tests/test_m043_sidecar_runtime_readiness.py`, `tests/test_m043_sidecar_packets.py`, `tests/test_m044_sidecar_architecture_guardrail.py`, `tests/test_m044_live_grobid_candidate_probe.py` |
| Binding | ADR-004, ADR-005, ADR-007, R053, R056, R059, D078, D079 |

### 2.6 Review & Readiness

| Role | Files |
|---|---|
| Library | `arxiv_archive/universal_kb_review_assistance.py` (no LLM approval authority), `arxiv_archive/universal_kb_substrate_rehearsal.py` (no real GraphDB), `arxiv_archive/reviewer_packet_prototype.py`, `arxiv_archive/graph_readiness_review.py`, `arxiv_archive/graph_readiness.py`, `arxiv_archive/graph_readiness_export.py`, `arxiv_archive/graph_readiness_extraction_gate.py`, `arxiv_archive/graph_readiness_persistence.py`, `arxiv_archive/graph_readiness_retrieval_validation.py`, `arxiv_archive/import_boundary_rehearsal.py`, `arxiv_archive/article_evidence_bridge.py` |
| Scripts | `scripts/render_reviewer_packet_prototype.py`, `scripts/audit_locator_evidence.py`, `scripts/audit_m036_real_corpus_smoke.py`, `scripts/audit_m042_connectivity_groups.py`, `scripts/replay_m031_import_boundary_rehearsal.py` |
| Verifiers | `scripts/verify_reviewer_packet_prototype.py`, `scripts/verify_m023_artifact_scaffold_gate.py`, `scripts/verify_m024_requirement_coverage.py`, `scripts/verify_m024_validation_evidence_closure.py`, `scripts/verify_m036_real_corpus_no_write_smoke.py`, `scripts/verify_m031_s05_closeout.py`, `scripts/verify_m031_validation_remediation.py` |
| Tests | `tests/test_m036_real_corpus_no_write_smoke.py`, `tests/test_m036_real_corpus_smoke_audit.py`, `tests/test_universal_kb_review_assistance.py` |
| Binding | ADR-002, ADR-004, ADR-006, R019, R023, R038, R051, R056, R058, R059 |

### 2.7 Durable Queue

| Role | Files |
|---|---|
| Library | `arxiv_archive/universal_kb_queue.py` (SQLite, WAL, leases, heartbeats), `arxiv_archive/universal_kb_rehearsal.py` |
| Tests | `tests/test_universal_kb_rehearsal.py`, `tests/test_universal_kb_architecture_guards.py` |
| Verifiers | `scripts/verify_m035_universal_kb_prototype.py` |
| Binding | ADR-003, R054, R055, D073 |

### 2.8 Trajectory & Ops

| Role | Files |
|---|---|
| Library | `arxiv_archive/universal_kb_smoke.py` (unified CLI: select/run/audit/verify/all), `arxiv_archive/validation_batch_state.py`, `arxiv_archive/validation_batch_workflow.py`, `arxiv_archive/validation_batch_provenance.py`, `arxiv_archive/validation_logging.py`, `arxiv_archive/article_batch_validation.py` |
| Scripts | `scripts/check_project_trajectory.py` (M045), `scripts/sync_codebase_memory_governance.py` (M038/M039), `scripts/run_m036_real_corpus_no_write_smoke.py` |
| Verifiers | `scripts/verify_m028_hermes_digest_projection.py`, `scripts/verify_m028_universal_loader_evidence_bundles.py`, `scripts/verify_m028_source_metadata_adapters.py`, `scripts/verify_m028_smoke_closeout.py`, `scripts/verify_m029_unified_loader_runtime_smoke.py`, `scripts/verify_m029_unified_readiness.py`, `scripts/verify_m029_unified_replay.py`, `scripts/verify_m030_*.py` (5 files), `scripts/verify_m034_*.py` (8 files), `scripts/verify_m036_real_corpus_no_write_smoke.py`, `scripts/verify_m044_sidecar_architecture_guardrail.py` |
| Tests | `tests/test_universal_kb_smoke_cli.py`, `tests/test_codebase_memory_governance.py`, `tests/test_m045_project_trajectory.py` |
| Binding | ADR-005, D075, D076, D079, D080, R033, R037, R062, R063, R064, R065 |

## 3. Module ↔ ADR ↔ Verifier Table (excerpt)

| Module / File | Primary ADR | Primary Verifier | Notes |
|---|---|---|---|
| `universal_kb_contracts.py` | ADR-000, ADR-004 | `verify_m035_universal_kb_prototype.py` | frozen stdlib dataclasses, SafetyFlags |
| `universal_kb_queue.py` | ADR-003 | `verify_m035_universal_kb_prototype.py` | SQLite durable queue |
| `universal_kb_sidecar_boundary.py` | ADR-004 | `verify_m035_universal_kb_prototype.py` | Adaptix anti-corruption |
| `universal_kb_review_assistance.py` | ADR-006 | `verify_m035_universal_kb_prototype.py` | no LLM approval authority |
| `universal_kb_substrate_rehearsal.py` | ADR-002, ADR-005 | `verify_m035_universal_kb_prototype.py` | no real GraphDB |
| `universal_kb_smoke.py` | ADR-005, ADR-004 | `verify_m036_real_corpus_no_write_smoke.py` | unified CLI |
| `check_project_trajectory.py` | D080 | self-check | 7 dimensions, drift flags |
| `sync_codebase_memory_governance.py` | D075, D076 | self-check | generated mirror |
| `verify_m044_sidecar_architecture_guardrail.py` | ADR-005, D079 | self-check | preflight required |
| `verify_m035_universal_kb_prototype.py` | ADR-003, ADR-004, ADR-005 | self-check | M035 prototype |
| `verify_m036_real_corpus_no_write_smoke.py` | ADR-005, D072-D074 | self-check | M036 smoke |
| `verify_m033_*.py` (5 files) | ADR-004, ADR-007 | self-check | M033 parsers + patterns |
| `verify_m034_*.py` (8 files) | ADR-000, ADR-002, ADR-003, ADR-006 | self-check | M034 decision package |
| `full_text.py` | R014 | M001 cron test | local full-text ingestion |
| `hybrid_retrieval.py` | R019 | M003 S06 tests | fixture-level baseline |
| `dspy_extraction.py` | R020, R021, R041 | n/a (disabled) | DSPy not enabled |
| `scientific_extraction.py` | R017 | contract tests | Claim/Entity/Relation |
| `ladybug_client.py` | ADR-002 (deferred) | substrate rehearsal only | experimental substrate |
| `evaluation.py` | R020 | M003 S07 metrics | eval fixtures first |
| `minimax_structured.py`, `minimax_usage.py` | R042-R045, D074 | M014-M017 verifiers | MiniMax helper |
| `article_artifact_minimax.py` | R051 | M023 verifier | MiniMax bounded helper |

## 4. Module Map Hotspots

### 4.1 Most-tested modules (top by test count)

| Module | Tests | Note |
|---|---|---|
| `universal_kb_*` (5 files) | 30+ tests | M035 contracts, queue, boundary, review, substrate, smoke |
| `chunk_import_contract.py` | 20+ tests | M005 chunking deepening |
| `validation_batch_*.py` (3 files) | 15+ tests | M007-M010 batch workflow |
| `graph_readiness_*.py` (6 files) | 15+ tests | M011 semantic gate, M022 repair |

### 4.2 Verifier coverage by milestone

| Milestone | Verifiers | Tests |
|---|---|---|
| M001-M020 | 35+ | 60+ |
| M022-M029 | 25+ | 40+ |
| M030-M031 | 5 | 10+ |
| M033-M039 | 15+ | 25+ |
| M040-M045 | 8+ | 15+ |
| **Total** | **64** | **154** |

## 5. Reverse ADR Audit (code-level checks)

Re-checking that the code does not violate any binding ADR:

| ADR | Expected invariant | Code finding |
|---|---|---|
| ADR-000 | universal KB, scientific articles first | no paper-only overfitting found — PASS |
| ADR-002 | no final GraphDB selection | no `from falkordb` / `from helixdb` imports — PASS |
| ADR-003 | durable lazy async evidence pipeline | queue + heartbeat + WAL present — PASS |
| ADR-004 | sidecars as candidate evidence only | `universal_kb_sidecar_boundary.py` enforces — PASS |
| ADR-005 | no direct parser to GraphDB | no GraphDB write paths in src/ — PASS |
| ADR-006 | agents do not orchestrate | `universal_kb_review_assistance.py` is diagnostic-only — PASS |
| ADR-007 | quant-mind as pattern source | no `quantmind` / `llmquant` imports in src/ or scripts/ — PASS |

**Result: 0 reverse ADR violations** at code level. The M044 guardrail is the durable runtime enforcement.

## 6. Known Module-Level Limitations

1. **M036 verifier rerun can dirty tracked evidence files** (M036 lesson learned). Future design should prefer temp artifact dirs.
2. **M035 verifier is dependent on `.gsd/DECISIONS.md` extension** beyond M034 R/D inventory. Verifier should use stable M034 snapshot (M035 lesson).
3. **`codebase-memory-mcp ingest_traces` does not implement runtime edge creation** despite accepting calls (M039 lesson). Avoid claiming native graph ingestion.
4. **DSPy optimizer remains disabled** per R020, R021, R041. `dspy_extraction.py` exists but is not activated in any milestone.
5. **GROBID live probe produced 1/6 success** in M044 due to missing local PDFs for 5 target articles.

## 7. Cross-References

- Architecture: `02-architecture-layers.md` (bounded contexts)
- Decisions: `03-adr-decisions.md` (ADRs, traceability)
- Safety: `05-evidence-safety.md` (fail-closed invariants at module level)
- Trajectory: `06-trajectory-ops.md` (M045 trajectory as ops layer)
- Assessment: `07-2026-assessment.md` (deep modules, observability, anti-patterns)

## 8. LLM Reading Notes (binding)

- **Treat this layer as the actual ground truth.** If a future plan contradicts this map, update the map (and re-run M045 trajectory check).
- **Read 02-architecture-layers.md first** for the bounded context abstraction, then this map for the file-level reality.
- **The 64 verifiers are the durable safety net.** Always read the relevant verifier before changing the module it covers.
- **Reverse ADR audit is a sanity check, not a substitute for the M044 guardrail.** Run the guardrail before any sidecar / graph-readiness work.
- **Tests are not the only proof.** Integration smoke + per-article continuity artifacts are equally load-bearing.
