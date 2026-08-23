# M400-xijx04 S05 — Multimodal closeout report (product-side)

**Milestone:** M400-xijx04 (revision-grounded multimodal evidence)
**Slice:** S05 (Multimodal Governor closure)
**Task:** T03 — Final multimodal closeout evidence
**Date:** 2026-08-18
**Type:** runtime-executable closeout evidence (candidate-only, D127)

This report links the final multimodal closeout matrix into one durable product-side
artifact: candidate counts, lineage coverage, fidelity conflicts, semantic graph task
outcomes, replay counters, and lock status. Every number below is bound to a fresh
runtime or Governor evidence run (see `## Evidence runs`).

---

## 1. Candidate counts

| Plane | Candidates | Source of truth |
|-------|-----------|-----------------|
| Figure (per revision, `figure_candidate_graph_task`) | **2** (one per revision v1/v2, revision-scoped sections) | `crates/da-application/tests/figure_candidate_test.rs` (`figure_candidate_graph_task`, persisted report `candidates=2`) |
| Table (`table_cell_fidelity_graph_task` fixture) | **4** (2 perfect × 6 cells + 1 conflicting × 4 cells + 1 no-visual-payload × 2 cells) | `data/graph_tasks/table_cell_fidelity.tasks.cypher` header + `crates/da-application/tests/table_candidate_test.rs` |
| Equation (per revision) | **2** | `data/graph_tasks/equation_candidate.tasks.cypher` (`@expect_value 2`) |
| Table cells (grounded, revision-bound) | **18** | `table_cell_fidelity.tasks.cypher` (`@expect_value 18`) + persisted summary `cells:18` |
| Canonical multimodal journey (same article, one process) | **3** (1 Figure + 1 Table + 1 Equation candidate) | `crates/da-application/tests/multimodal_pipeline_test.rs` |

## 2. Lineage coverage

| Contract | Rule | Covered by |
|----------|------|-----------|
| Visual candidate lineage | FigureCandidate requires revision (`revision_vid`), section (`section_vid`), artifact kind, detector identity, 4×sha256, caption, candidate metadata digest | `scripts/governor/checks/visual_candidates_require_graph_lineage.yaml` (positive/negative fixtures) + `figure_candidate_graph_task` grounded/revision/section queries (2/2/2 PASS) |
| Table cell provenance | TEI grounding (`tei_table_id`, `tei_source_digest`) + per-cell grid provenance (row/column) + digest; both-or-neither source span (R087 typed missingness) | `scripts/governor/checks/table_payload_requires_cell_provenance.yaml` + `table_cell_fidelity.tasks.cypher` (18 grounded cells, 4 revision-bound candidates) |
| Equation context coverage | `formula_id`, revision/section lineage (`section_path_id`, `section_ordinal`), typed representation (MathML/TeX/plain), both-or-neither span, deterministic identity | `scripts/governor/checks/equation_context_coverage.yaml` + `equation_candidate.tasks.cypher` (2 grounded, 2 revision-bound, 2 section-bound, 2 formula identity, 2 representation, 2 digest, 2 span) |

## 3. Fidelity conflicts

| Metric | Value | Evidence |
|--------|-------|----------|
| Typed conflicts (table) | **2** (`header_mismatch` + `numeric_mismatch`) | `table_candidate_test.rs` `typed_conflicts_materialize_as_conflict_nodes_with_verbatim_texts` (`conflicts_created=2`), persisted summary `conflicts:2` |
| Conflict plane | `TableCandidate -[:CONFLICTS_OVER]-> TableConflict`, never an overwrite (ADR-047) | `data/graph_tasks/table_cell_fidelity.tasks.cypher` header; `table_candidate_test.rs:485` |
| Coverage gaps | Counters on candidate node (`coverage_unmatched_*`), never conflict nodes, never invented cells (R087 no-invention) | `table_cell_fidelity.tasks.cypher` header + `table_candidate_test.rs` |

## 4. Semantic graph task outcomes

| Suite | Tasks | Outcome |
|-------|-------|---------|
| `figure_candidate.tasks.cypher` | 7 (grounded 2, revision-bound 2, section-bound 2, orphans 0, unlocked-import 0, locked 2, authoritative 0) | **7/7 PASS** on grounded graph; suite **fails** on empty graph (EMPTY) and on ghost orphan candidate (`figure_candidate_test.rs`) |
| `table_cell_fidelity.tasks.cypher` | 21 queries (18 grounded cells, 4 revision-bound, 2 conflicts, 0 unlocked, 0 authoritative, …) | **All PASS** in `table_cell_fidelity_graph_task`; empty graph and ghost candidate negative controls fail as designed |
| `equation_candidate.tasks.cypher` | 11 (2 grounded, 2 revision-bound, 2 section-bound, 2 identity/representation/digest/span, 0 orphans, 0 unlocked, 2 locked, 0 authoritative) | **All PASS** in `equation_candidate_graph_task`; negative controls fail as designed |
| Parser (`da-cli` `graph_tasks` module) | 31 tests incl. proptest (parse roundtrip, non-empty cypher, marker count == task count) | **31 passed, 0 failed** |

Per ADR-051 §8.1 the suite fails on EMPTY/ERROR — the negative controls prove the
suites are not vacuously green.

## 5. Replay counters

| Plane | Counter | Value | Evidence |
|-------|---------|-------|----------|
| Pipeline plane | Executor calls after identical replay | **1** per stage (all 7 stages `CacheHit` with exact `cache_key_sha256`; replay does not re-execute, no duplicate stage records) | `multimodal_pipeline_test.rs` canonical journey |
| Pipeline plane | Stage records after replay | **7** (one per step, no duplicates) | same |
| Materialization plane | Figure/Table/Equation replay | `applied=false`, `unchanged=true`, `nodes_created=0`, `edges_created=0`, diagnostic `UNCHANGED_REPLAY` | `multimodal_pipeline_test.rs:716-744` |
| Materialization plane | First application | figure `nodes_created=1/edges_created=2`; equation `nodes_created=1/edges_created=2`; table applied | same |
| Query plane | Duplicates after replay | exactly 1 candidate per type (figure/table/equation) | `multimodal_pipeline_test.rs:806-828` |

## 6. Lock status (D127 / ADR-053 / D238)

| Lock | Status | Evidence |
|------|--------|----------|
| `import_eligible=false` (D127) | **Locked everywhere** — data stage ledger (`load_pipeline_stage_ledger` asserts `!ledger.import_eligible`), figure/table/equation candidates, `MultimodalFingerprintSummary`, query plane (0 nodes with `import_eligible=true`) | `multimodal_pipeline_test.rs:111,636,657-672,789`; `figure_candidate_test.rs`; `table_candidate_test.rs`; `equation_candidate_test.rs` |
| `authoritative=false` | **Locked** — query plane returns 0 `authoritative=true` nodes per candidate type | `multimodal_pipeline_test.rs:799` |
| Optimizer auto-promotion (D238) | **Off** — no auto-promote path in S05; authority/import unlock records are negative fixtures in Governor contracts | `scripts/governor/checks/*.yaml` negative fixtures |
| Governor authority-lock rules | `no_import_eligible_true*` / `no_graph_writes*` high → blocked; `authority_unlock`/`import_unlock` in negative fixtures | Governor `--self-test` 65/65, `--phase active` violations=0 |

## 7. Review register closure

| Finding | Status | Maturity | Evidence |
|---------|--------|----------|----------|
| PIR-018 (figures) | **resolved** | 4 / 4 | M400 S01+S05 runtime + Governor evidence (`visual_candidates_require_graph_lineage`) |
| PIR-019 (tables) | **resolved** | 4 / 4 | M400 S02+S05 runtime + Governor evidence (`table_payload_requires_cell_provenance`) |
| PIR-020 (equations) | **resolved** | 3.5 / 3.5 | M400 S03+S05 runtime + Governor evidence (`equation_context_coverage`) |
| PIR-017 / PIR-021 | open (M401 agent readiness) | 0.5/3.5, 2.2/4 | untouched |
| Register total | 22 rows, 20 resolved, 2 open | — | `doc/review/pipeline-integrity-review.jsonl` |

## 8. Evidence runs (2026-08-18, refreshed 2026-08-18 T03 re-run)

All IDs below are live `gsd_exec` evidence runs from the current T03 attempt (post stale-worker lease takeover, same committed tree — `git status` clean, no file newer than this report).

| # | Command | Exit | Result | Evidence id |
|---|---------|------|--------|-------------|
| 1 | `cargo test -p da-application --test multimodal_pipeline_test` | 0 | 2 passed (admission gate + canonical journey) | gsd_exec 8f3f00e8 |
| 2 | `PYTHONPATH=scripts uv run --with pyyaml python -m governor --phase active --check-only` | 0 | violations=0 (verdict DRIFT_RISK, only pre-existing `resource_target_bloat`) | gsd_exec 3f3b23de |
| 3 | `PYTHONPATH=scripts uv run --with pyyaml python -m governor --validate-yaml` | 0 | 59 rules, 12 dimensions PASS | gsd_exec 3f3b23de |
| 4 | `PYTHONPATH=scripts uv run --with pyyaml python -m governor --self-test` | 0 | 65 passed, 0 failed (18 multimodal contract tests) | gsd_exec 3f3b23de |
| 5 | `bash scripts/verify_rust_architecture.sh` | 0 | PASS: fmt + clippy (deny warnings) + hexagonal invariant + 50 suites `test result: ok`, 1618 tests, 0 FAILED (`GUARDRAIL_EXIT=0`) | gsd_exec 2471cd64 |
| 6 | `cargo test -p da-cli --lib graph_tasks` | 0 | 31 passed (parser + proptest) | gsd_exec 8f3f00e8 |
| 7 | `cargo test -p da-application --test pipeline_cache_audit_test --test pipeline_ledger_audit_test` | 0 | 3 + 2 passed (cache identity + ledger canonical/negative) | gsd_exec 1968a917 + 8f3f00e8 |

**Per-crate regression scope** (pipeline_ledger / pipeline / production / load): covered by
`da-application --tests` inside guardrail run #5 (incl. `pipeline_ledger_audit_test`,
`pipeline_cache_audit_test`, `multimodal_pipeline_test`, `figure_candidate_test`,
`table_candidate_test`, `equation_candidate_test`) plus explicit runs #6/#7.

## 9. Candidate-only boundary (unchanged by this closeout)

- Candidates are candidate evidence, never graph truth: `authoritative=false`,
  `import_eligible=false` (D127), optimizer auto-promotion off (D238).
- GROBID TEI / sidecar output remains sidecar opinion, not graph truth (candidate
  evidence only).
- No import unlock, no auto-promote, no authority grant anywhere in S05 evidence.
- R094 is ready for `validated` at phase transition (orchestrator-owned
  `gsd_requirement_update`; hard-blocked in execute-task lane by design).
