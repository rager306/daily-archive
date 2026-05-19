# Source acquisition readiness delta report

## Summary

S02 addressed the source-readiness blocker found in S01. The 30-paper corpus started with only 10/30 Markdown-ready papers. After bounded acquisition and targeted repair, the corpus is now 30/30 Markdown-ready.

This means S03 can run a meaningful 30-paper chunking/import-model deviation analysis over Markdown sources instead of mostly measuring missing-source blockers.

## Readiness delta

| Metric | S01 before acquisition | S02 after acquisition | Delta |
|---|---:|---:|---:|
| Total selected papers | 30 | 30 | 0 |
| Markdown-ready papers | 10 | 30 | +20 |
| Missing Markdown papers | 20 | 0 | -20 |
| Cached PDFs | 2 | 8 | +6 |

## Acquisition notes

- S01 identified 20 originally missing Markdown papers.
- A first unbounded acquisition attempt was cancelled because bulk PDF/Docling fallback was too slow for the batch.
- The helper was updated to support fast arxiv2md-only batch acquisition and to distinguish original S01 missing counts from current run attempts.
- Fast acquisition brought the corpus to 29/30 Markdown-ready.
- The final blocker, `2001.00186v1`, had low-quality/empty arxiv2md output and required one targeted wall-clock-bounded Docling repair.
- The final refreshed summary reports `ready_for_markdown_scan_count=30` and `still_missing_markdown_count=0`.

## Remaining caveats

- PDF availability is still partial: 8/30 cached PDFs are present.
- The corpus is now ready for Markdown-based deviation analysis, not multimodal/PDF-complete analysis.
- Targeted Docling repair worked for one paper, but bulk Docling/PDF fallback should remain bounded and targeted; it should not be enabled blindly for larger scans.
- This slice does not authorize KG import, embeddings, vector generation, or production LadybugDB writes.

## Safety boundary

The source acquisition summary and diagnostics keep all safety flags closed:

- `raw_text_included=false`
- `chunk_text_included=false`
- `raw_binary_included=false`
- `base64_included=false`
- `embeddings_included=false`
- `vectors_included=false`
- `secrets_included=false`
- `optimizer_traces_included=false`
- `production_import_attempted=false`
- `ladybugdb_written=false`

## S03 recommendation

Proceed to S03 as a Markdown-based 30-paper deviation analysis. S03 should compare M005 10-paper baseline against the 30-paper corpus and explicitly separate:

1. chunking/import-model deviations over all 30 Markdown-ready papers;
2. PDF/source preservation caveats, since PDFs are still available for only 8/30;
3. conversion-method caveats, especially the targeted Docling repair case.

S03 should not claim positive KG import readiness unless a reviewed non-zero import-eligible subset is created separately.

## Evidence

- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json`
- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json`
- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl`
