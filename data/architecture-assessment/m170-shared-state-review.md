# M170 Shared State Write Review

## Verdict

**No immediate shared-state code remediation is required in S04.**

The four records classified as `shared-state` remain intentionally visible, but source review shows they are not the same issue as M169's final unknown stable cache writes. They are either caller-owned outputs, run-scoped outputs, or legacy one-shot summary/report artifacts. M170 should not hide them by broad scanner reclassification and should not add speculative locks here.

## Evidence

Inventory source:

- `data/architecture-assessment/m170-write-path-inventory.json`
- `data/architecture-assessment/m170-write-path-inventory.md`

Source and callsite evidence:

- exact shared-state extraction: `gsd_exec[32861fcd-313d-43bb-a59d-5df40ba5443c]`
- targeted callsite search: `gsd_exec[f0843798-60f8-4fa1-99e5-0ec39d677682]`
- catalog report search: `gsd_exec[6881e47b-bae2-47be-b4db-2f523fc51cba]`

## Record dispositions

| Record | Target | Observed ownership | Disposition | Rationale |
|---|---|---|---|---|
| `src/research_graph/application/validation/batch_state.py:252` | `output_path` | Run-scoped validation batch artifact | **Policy-only, safe by run scope** | Callers write to `artifact_dir / "batch-state.json"` or `output / "batch-state.json"`; initialization scopes by `batch_artifact_dir(output_dir, batch_id)`. Later workflow phases intentionally replace the same batch state as a single workflow owner. |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py:540` | `summary_path` | Legacy M056 ingest summary | **Policy-only, legacy one-shot summary** | Called by `scripts/ingest_m056_corpus.py` with `data/r024-218-document-corpus-v1/ingest-summary.json`; this is a replay/ingest summary artifact, not an active multi-worker cache. |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py:935` | `report_path` | M061 ingest report renderer | **Policy-only, report artifact** | `render_report(...)` has default `artifacts/m061-2hop/s04-ingest-report.md`; M168 already hardened canonical catalog writes. The report write is human-readable evidence, not the catalog state mutation. |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py:183` | `index_path` | Caller-owned review index | **Policy-only, caller-owned pair** | CLI requires `--review-output` and `--review-index` together; tests pass temp paths. The index is part of a review sample output pair, not shared package state. |

## What S04 does not change

- No scanner category changes.
- No broad lock policy.
- No code edits.
- No claim that all shared-state writes are globally concurrency-proof.

## Downstream action

S05-S08 should focus on the explicit M169 residual risk: **same-key stable CLI and PDF cache write coordination**.

S04 does not create new code targets. The appropriate downstream decision is:

1. compare atomic-only, lock-file, and compare-and-swap style policies for same-key stable cache writes;
2. implement code only if S05 proves likely concurrent same-key writers or stale overwrite detection needs;
3. otherwise close cache coordination as a documented atomic-only policy with future activation triggers.

## Residual risks

1. Validation batch state intentionally overwrites the same `batch-state.json` across phases; this is safe for single workflow ownership but not a multi-owner contract.
2. Legacy summary/report artifacts may still be overwritten by reruns; this is acceptable for evidence regeneration and should not be treated as active shared mutable state.
3. The inventory should keep `shared-state=4` visible until a future scanner can express `run-owned replacement`, `legacy evidence regeneration`, or `caller-owned paired output` without hiding real shared-state risk.

## Acceptance contract impact

S04 satisfies the M170 architecture backlog target by reviewing all four shared-state records and documenting policy-only closure. It preserves the M170 contract:

```text
write_path_unknown=0
shared-state records remain visible
no broad scanner weakening
no speculative locks
```
