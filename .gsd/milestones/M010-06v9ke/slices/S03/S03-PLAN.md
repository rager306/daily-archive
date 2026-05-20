# S03: Run provenance verified next plus ten scan

**Goal:** Run the materialized M010 source-ready batch scan with active M010 lineage, real provenance entry, and verify-artifacts freshness proof.
**Demo:** After this slice, the next +10 scan artifacts are active-lineage stamped and verified fresh by provenance.

## Must-Haves

- Scan uses S02 materialized source-ready batch state.
- Scan command uses --milestone-id M010-06v9ke.
- Provenance JSONL records real scan input/output hashes and expected metadata.
- verify-artifacts returns fresh for real scan artifacts.
- Scan artifacts contain milestone_id=M010-06v9ke and matching batch_id.
- import_eligible_chunk_count remains 0 or blocks progression.
- No production import or LadybugDB write occurs.

## Proof Level

- This slice proves: Quota guard, scan guard, provenance freshness guard, focused regression.

## Integration Closure

Consumes S02 source-ready batch state and produces scan/provenance/freshness artifacts for S04 review.

## Verification

- Adds scan summary, diagnostics, delta/outlier reports, provenance JSONL, freshness report, and scan report with active lineage.

## Tasks

- [x] **T01: Run active lineage scan** `est:medium`
  Run validation-batch scan over the materialized S02 source-ready batch state with active M010 milestone lineage. Persist scan response, summary, diagnostics, delta, outlier, manifest, and report.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json`, `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/delta-report.json`, `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/outlier-report.json`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json').read_text())
assert s['paper_count']==10
assert s['milestone_id']=='M010-06v9ke'
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('active-lineage-scan-ok')
PY

- [x] **T02: Record and verify scan provenance** `est:medium`
  Create a real scan provenance JSONL entry for the S03 scan inputs/outputs with expected milestone_id and batch_id metadata, then run verify-artifacts and persist freshness report.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-provenance.jsonl`, `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json && uv run python - <<'PY'
import json
from pathlib import Path
r=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json').read_text())
assert r['verdict']=='fresh'
print('scan-provenance-fresh-ok')
PY

- [x] **T03: Write scan report and final guard** `est:small`
  Run final S03 scan guard across quota, scan counts, active lineage, provenance freshness, and safety flags. Write validation scan report.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S03/validation-scan-report.md`, `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-guard.json`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-guard.json').read_text())
assert g['paper_count']==10
assert g['freshness_verdict']=='fresh'
assert g['milestone_id']=='M010-06v9ke'
assert g['production_import_attempted'] is False
assert g['ladybugdb_written'] is False
print('scan-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json
- .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/delta-report.json
- .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/outlier-report.json
- .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-provenance.jsonl
- .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json
- .gsd/milestones/M010-06v9ke/slices/S03/validation-scan-report.md
- .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-guard.json
