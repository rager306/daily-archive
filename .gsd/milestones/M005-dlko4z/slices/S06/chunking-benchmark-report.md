# S06 Chunking Benchmark Report

## Verdict

S06 benchmark completed as a redacted dry-run and documents a blocker for positive S07 import rehearsal.

No benchmarked method is safe for trusted KG import or a positive isolated import rehearsal because all compared chunks/candidates remain refused:

```json
{
  "total_chunk_count": 2471,
  "total_import_eligible_chunk_count": 0,
  "total_refused_chunk_count": 2471,
  "recommendation_status": "review_required"
}
```

## Methods compared

| Method | Role | Result |
|---|---|---|
| `baseline_pageindex_semanticchunk` | S02 current baseline | Retrieval-only, no spans/annotations/assets, zero import eligibility. |
| `structure_aware_control` | S03/S04/S05 deterministic control | Full source-span and annotation coverage, asset linkage available, still all chunks refused/import-blocked. |
| `simple_section_window_estimate` | Bounded deterministic estimate from S05 source/assets | Useful comparison estimate only, not a real chunker and not import-ready. |

## Key numbers

- Methods: 3
- Total compared chunks/candidates: 2,471
- Import-eligible chunks: 0
- Refused chunks: 2,471
- Missing-source caveat: `missing_original_pdf=8` in S05 appears twice in aggregate because it affects two benchmark methods that consume S05 source context.

Aggregate route distribution:

```json
{
  "citation_graph": 22,
  "claim_extraction": 245,
  "metadata_graph": 4,
  "method_extraction": 136,
  "retrieval_only": 1988,
  "table_extraction": 76
}
```

Aggregate state distribution:

```json
{
  "ok_for_retrieval_only": 1524,
  "repair_required": 947
}
```

## What improved over baseline

The structure-aware control improves observability and reviewability relative to the baseline:

- source-span coverage rises from 0.0 to 1.0;
- annotation coverage rises from 0.0 to 1.0;
- asset-linkage coverage becomes measurable;
- route/type/refusal distributions become explicit;
- table, figure, equation, reference, metadata, claim, and method candidates are distinguishable.

This is meaningful benchmark progress, but it is not import readiness.

## Why S07 positive import rehearsal remains blocked

S07 positive/trusted import rehearsal requires at least one reviewed import-eligible subset. S06 found none.

Reasons include:

- baseline chunks are retrieval-only;
- structure-aware chunks remain repair-required or retrieval-only;
- candidate routes require review;
- table/figure/equation/reference assets are linked-not-extracted;
- external real chunking libraries were not executed;
- 8 PDFs remain missing from current source paths.

## Independent review

Independent review returned BLOCK for S07 positive/import rehearsal.

Review artifact:

```text
.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-summary.md
```

Review conclusion:

> The benchmark artifacts are valid redacted dry-run evidence, but no method can safely unblock S07 positive/import rehearsal because all 2,471 candidates are refused and import eligibility is zero.

## Authoritative diagnostics

Future agents should inspect these first:

- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json` — aggregate comparison, import eligibility, recommendation status, safety flags.
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl` — method-level route/type/state/refusal/coverage diagnostics.
- `.gsd/milestones/M005-dlko4z/slices/S06/review/chunking-benchmark-review-samples.md` — bounded reviewer-facing method comparison and review questions.
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-summary.md` — independent review verdict.

## Recommended next decision

Before S07, choose one of two paths:

1. **Re-scope S07 as a negative import-boundary rehearsal**: prove the isolated importer rejects all current candidates safely and writes no production KG data.
2. **Add a remediation slice before S07**: create a small reviewed import-eligible subset, likely by repairing/approving a few structure-aware claim/method/table candidates with evidence paths.

Path 1 is safer and consistent with current evidence. Path 2 is required only if S07 must demonstrate a positive import path.

## What remains unproven

S06 does not prove:

- any chunk is trusted/import-ready;
- semantic/vector retrieval quality;
- entity or relation extraction quality;
- real external chunking library quality;
- extracted figure/table asset quality;
- production LadybugDB writes;
- broad corpus scaling.
