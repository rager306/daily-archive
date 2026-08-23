# M031 Validation Remediation Dossier

Schema: `m031-validation-remediation-evidence.v1`
Milestone: `M031-vwpd8e`
Selection: `m031-catalog-backed-replay-v1`
Metadata-only: true

## Reader Action

Use this dossier as the M031 validation-rerun input for S06. Read the evidence JSON for machine-checkable rows, read this report for the human handoff, and do not interpret any row as graph-import approval or global requirement validation.

## Source Artifact Audit

- `continuity_audit`: `data/article_corpora/m031-catalog-backed-replay-v1/m031-continuity-audit.json`
- `progression_matrix`: `data/article_corpora/m031-catalog-backed-replay-v1/progression-matrix.json`
- `replay_closeout`: `data/article_corpora/m031-catalog-backed-replay-v1/replay-closeout-summary.json`
- `review_events`: `data/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence/independent-review-events.jsonl`
- `s02_assessment`: `.gsd/milestones/M031-vwpd8e/slices/S02/S02-ASSESSMENT.md`
- `s02_summary`: `.gsd/milestones/M031-vwpd8e/slices/S02/S02-SUMMARY.md`
- `s02_uat`: `.gsd/milestones/M031-vwpd8e/slices/S02/S02-UAT.md`
- `s05_closeout`: `data/article_corpora/m031-catalog-backed-replay-v1/s05-closeout-summary.json`

## S02 Assessment Reconciliation

- Stale S02 assessment failure detected: `True`
- Fresh `65 passed` evidence present: `True`
- Fresh `65 passed` evidence sources: `.gsd/milestones/M031-vwpd8e/slices/S02/S02-SUMMARY.md`, `.gsd/milestones/M031-vwpd8e/slices/S02/S02-UAT.md`
- Full-repo pytest collection debt classified outside S02 UAT: `True`

## Requirement Coverage

| Requirement | Coverage status | Validated | Validation claim allowed |
|---|---|---|---|
| R024 | m031_scoped_rechecked | False | False |
| R027 | m031_scoped_rechecked | False | False |
| R029 | m031_scoped_rechecked | False | False |
| R040 | m031_scoped_rechecked | False | False |
| R050 | m031_scoped_rechecked | False | False |

## Canonical Verification Classes

| Class | Status | Evidence paths |
|---|---|---|
| Contract | covered_for_validation_rerun | .gsd/milestones/M031-vwpd8e/slices/S02/S02-SUMMARY.md, data/article_corpora/m031-catalog-backed-replay-v1/s05-closeout-summary.json, data/article_corpora/m031-catalog-backed-replay-v1/m031-continuity-audit.json |
| Integration | covered_for_validation_rerun | .gsd/milestones/M031-vwpd8e/slices/S02/S02-SUMMARY.md, data/article_corpora/m031-catalog-backed-replay-v1/s05-closeout-summary.json, data/article_corpora/m031-catalog-backed-replay-v1/m031-continuity-audit.json |
| Operational | covered_for_validation_rerun | .gsd/milestones/M031-vwpd8e/slices/S02/S02-SUMMARY.md, data/article_corpora/m031-catalog-backed-replay-v1/s05-closeout-summary.json, data/article_corpora/m031-catalog-backed-replay-v1/m031-continuity-audit.json |
| UAT | covered_for_validation_rerun | .gsd/milestones/M031-vwpd8e/slices/S02/S02-SUMMARY.md, data/article_corpora/m031-catalog-backed-replay-v1/s05-closeout-summary.json, data/article_corpora/m031-catalog-backed-replay-v1/m031-continuity-audit.json |

## Safe Claims

### Requirement claims
- R024 has M031-scoped remediation coverage evidence only; no global status is changed.
- R027 has M031-scoped remediation coverage evidence only; no global status is changed.
- R029 has M031-scoped remediation coverage evidence only; no global status is changed.
- R040 has M031-scoped remediation coverage evidence only; no global status is changed.
- R050 has M031-scoped remediation coverage evidence only; no global status is changed.

### Verification-class claims
- Contract rerun evidence is metadata-only and scoped to M031 validation remediation.
- Integration rerun evidence is metadata-only and scoped to M031 validation remediation.
- Operational rerun evidence is metadata-only and scoped to M031 validation remediation.
- UAT rerun evidence is metadata-only and scoped to M031 validation remediation.

### Graph/import boundary claim
- Absent completed-review verdict evidence remains refusal evidence, not graph/import eligibility.

## Forbidden Claims

- Do not claim that S06 validates global requirement status for R024/R027/R029/R040/R050.
- Do not assert graph-import readiness, KG-import readiness, trusted fact promotion, or production-import authorization for M031.
- Do not claim completed-review evidence exists when the independent review events still enforce refusal.
- Do not claim network fetches, model calls, raw article payload handling, production graph import, or LadybugDB writes occurred.

## Fail-Closed Safety

S06 does not enable production graph import or LadybugDB writes. Graph/import/LadybugDB/production/model/network/write activity remains false, accepted/import-eligible counts remain zero, and requirement status changes are not performed.

## Milestone Validation Handoff Snippets

- S02 stale assessment closeout: stale S02 assessment failure is reconciled by fresh `65 passed` S02 summary/UAT evidence, while unrelated full-repo pytest collection debt remains outside scoped S02 UAT.
- Requirement coverage: R024/R027/R029/R040/R050 have M031-scoped remediation coverage rows only; `validated` and `validation_claim_allowed` remain false.
- Verification classes: Contract, Integration, Operational, and UAT rows are covered for validation rerun using metadata-only evidence paths.
- Safety boundary: no raw article text, chunk text, vectors, embeddings, model traces, network traces, write traces, secrets, PDF bytes, or base64 payloads are included.

## Stable Diagnostics

- `M031_VALIDATION_REMEDIATION_FULL_REPO_COLLECTION_DEBT_OUTSIDE_S02_UAT`: 1
- `M031_VALIDATION_REMEDIATION_STALE_S02_ASSESSMENT_RECONCILED`: 1

## Failure Modes
- Filesystem inputs: missing or malformed local JSON/JSONL/Markdown artifacts return stable diagnostics and nonzero exit before any requested write.
- Unsafe flags or positive graph/import/LadybugDB/production/model/network/write claims fail closed before outputs are written.
- Absent completed-review verdict evidence is treated as refusal evidence, not positive eligibility.

## Load Profile
- Expected load: fixed M031 artifact set: S02 prose, replay closeout, S05 closeout, progression matrix, continuity audit, and two review-event rows
- 10x breakpoint: local JSON/Markdown parsing and recursive metadata scanning saturate first at roughly 10x rows; no network, subprocess, model, graph, or LadybugDB path exists
- Protection: single-pass bounded summaries, counters, stable diagnostic codes, and no raw payload reads or database writes

## Negative Tests
- stale S02 assessment without fresh 65-pass evidence
- missing requirement rows
- missing canonical class rows
- unsafe true flags
- raw payload or key leakage
- permissive graph/import claims
- malformed diagnostics
- path traversal or out-of-corpus output paths

## Rerun Commands

- `uv run python scripts/verify_m031_validation_remediation.py --validate-only`
- `uv run pytest tests/test_m031_validation_remediation.py -q`
