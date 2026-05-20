# Independent review — M008 first new +10 quota-filled scan

Verdict: FLAG

## Evidence checked

- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`
- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/delta-report.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/outlier-report.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/validation-scan-report.md`

## Findings

### Medium — quota-fill gate proves this happy path, not shortage automation

The quota artifact proves this batch met the gate before scan:

- `target_count=10`
- `attempted_count=10`
- `accepted_ready_count=10`
- `rejected_count=0`
- `shortage_count=0`
- `scan_allowed=true`

This is enough for the current M008 scan because the batch is fully source-ready, but it does not prove automatic top-up behavior when `accepted_ready_count < target_count`. No M008 artifact demonstrates replacement selection, max-attempt bounds, or blocked-scan behavior under shortage.

### Medium — source readiness is supported, PDF completeness remains weak

S02 final preflight supports Markdown scan readiness:

- `paper_count=10`
- `markdown_present_count=10`
- `markdown_quality_accepted_count=10`
- `ready_for_markdown_scan_count=10`
- `blocker_count=0`

But PDF completeness remains partial:

- `pdf_present_count=1`
- `pdf_missing_count=9`

This is acceptable for the current Markdown-based scan only if kept as a caveat.

### Medium — scan evidence is useful but has stale milestone metadata

The scan summary supports a safe operational scan:

- `paper_count=10`
- `valid_package_count=10`
- `chunk_count=1591`
- `import_eligible_chunk_count=0`
- `refused_chunk_count=1591`
- `production_import_attempted=false`
- `ladybugdb_written=false`

However, `validation-scan-summary.json` contains stale metadata:

```json
"milestone": "M006-638rza"
```

The artifact path and batch context are M008. This weakens traceability and should be corrected before relying on automated milestone labels in future scan artifacts.

### Low — outliers are correctly exposed

The outlier report lists 6 outliers, including:

- `1701.00001`: `zero_chunks`
- `2001.00254v1`: `high_chunk_count`, `table_heavy`
- several `claim_candidate_heavy` / `table_heavy` papers

These outliers are useful operational evidence and should not be treated as semantic KG readiness.

### Low — redaction/no-write/no-import claims are supported

Reviewed artifacts consistently report false for raw text, chunk text, raw binary/base64, embeddings/vectors, secrets, optimizer traces, production import, and LadybugDB writes.

## Risks and caveats

- Current quota-fill evidence is a success-path proof, not shortage/top-up proof.
- No automatic bounded replacement/top-up loop exists yet.
- PDF completeness is only 1/10.
- Scan summary has stale `milestone: M006-638rza` metadata inside an M008 artifact.
- One paper produced zero chunks.
- Import eligibility remains zero, so positive KG import remains blocked.

## Recommendation

Do not block completion of M008: the current first new +10 scan is safe, redacted, no-write/no-import, and quota-gated.

Before running another +10 batch, add bounded top-up automation or an equivalent CLI gate that proves shortage handling:

- reject/shortage detection;
- deterministic replacement candidate selection;
- max-attempt or max-candidate bounds;
- blocked-scan behavior when `accepted_ready_count < target_count`;
- traceable milestone/batch metadata in scan artifacts.
