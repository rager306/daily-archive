# M192 GitNexus Graph Scope

## Verdict

**M192 scope is review-only graph-readiness and import-eligibility boundary validation.**

M192 must not perform production graph import, LadybugDB production writes, direct extractor-to-graph writes, production retrieval claims, or optimizer activation.

## GitNexus evidence

| Evidence | Result |
|---|---|
| Query: `graph readiness review import eligibility manifest synthesis completed review output_contract_completed` | Found import-boundary rehearsal and review-adjacent processes, including `build_m031_import_boundary_rehearsal`. |
| Context: canonical source UID | `Function:src/research_graph/infrastructure/staging/import_boundary.py:build_m031_import_boundary_rehearsal` participates in `proc_273_build_m031_import_bo` and `proc_274_build_m031_import_bo`. |
| Impact: canonical source UID upstream | LOW risk, impactedCount=0, direct=0, processes_affected=0, modules_affected=0. |
| Query: local executable surfaces | Found `src/research_graph/infrastructure/staging/import_boundary.py`, `scripts/replay_m031_import_boundary_rehearsal.py`, and graph/import-boundary tests. |

## Canonical seam

Use canonical source seam:

- `src/research_graph/infrastructure/staging/import_boundary.py`
- UID: `Function:src/research_graph/infrastructure/staging/import_boundary.py:build_m031_import_boundary_rehearsal`

Exclude archived/mutant copies from M192 planning and execution claims:

- `archive/package-rename-waves/...`
- `mutants/...`
- `__pycache__/...`

## Planned local surfaces

### Review post-check

Project convention requires graph-readiness review artifact post-check before manifest synthesis:

```bash
uv run python -m arxiv_archive.graph_readiness_review \
  --review-dir <review-dir> \
  --events <events.jsonl> \
  --validate-only \
  --require-completed-review
```

If local module or completed-review inputs are unavailable, M192 records a fail-closed blocker and does not promote import eligibility.

### Import-boundary rehearsal

Candidate local surfaces:

- `scripts/replay_m031_import_boundary_rehearsal.py`
- `tests/test_m031_import_boundary_rehearsal.py`
- `tests/test_import_boundary_rehearsal.py`
- `src/research_graph/infrastructure/staging/import_boundary.py`
- `src/research_graph/workflows/import_boundary_rehearsal.py`

### Graph-readiness tests

Candidate local tests:

- `tests/test_graph_readiness_contract.py`
- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_extraction_gate.py`
- `tests/test_graph_readiness_manifest.py`
- `tests/test_graph_readiness_persistence.py`
- `tests/test_graph_readiness_retrieval_validation.py`
- `tests/test_graph_readiness_review.py`

## Scope rule

M192 may validate fail-closed review/import boundaries. It may not claim graph readiness or import eligibility unless completed-review artifacts pass the required post-check and all generated outputs preserve explicit safety flags.
