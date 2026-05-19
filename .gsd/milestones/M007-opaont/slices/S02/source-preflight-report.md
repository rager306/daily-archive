# Source preflight dry-run report

## Summary

S02 ran the new M007 `validation-batch init` and `validation-batch preflight` commands against the existing M006 30-paper corpus manifest.

The run produced local redacted batch artifacts only. It did not acquire sources, convert PDFs, run scans, import KG facts, or write to LadybugDB.

## Evidence

| Metric | Value |
|---|---:|
| Papers | 30 |
| Markdown present | 30 |
| Markdown quality accepted | 30 |
| Ready for Markdown scan | 30 |
| PDFs present | 8 |
| PDFs missing | 22 |
| Diagnostics | 20 |
| Blockers | 0 |
| Warnings | 20 |
| Production import attempted | false |
| LadybugDB written | false |

## Diagnostics

All 20 diagnostics are warnings with code:

```text
ready_with_missing_markdown_risk_tag
```

This is expected and useful. It surfaces the exact contradiction called out by the M006 independent review: the expansion papers are now Markdown-scan-ready after acquisition/repair, but still carry historical `missing_markdown` risk tags from the original corpus selection/audit state.

The warnings are not blockers for the current preflight because Markdown exists and is accepted for scan. Future automation should either mark these risk tags as historical or add a resolved-state field so later reviewers do not confuse original source gaps with current readiness.

## Artifact paths

- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/init-response.json`
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/preflight-response.json`
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/batch-state.json`
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json`
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-diagnostics.jsonl`
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/batches/m007-s02-thirty-paper/selection-manifest.json`

## Result

S02 proves the validation-batch CLI can initialize and preflight the 30-paper corpus deterministically, producing the source readiness and contradiction artifacts needed by S03 scan automation.
