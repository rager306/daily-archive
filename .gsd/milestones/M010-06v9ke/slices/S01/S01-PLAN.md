# S01: Select next gated plus ten corpus

**Goal:** Select the next deterministic +10 paper corpus excluding prior M006 and M008 validation corpora, with redacted availability evidence.
**Demo:** After this slice, a new +10 manifest exists with no overlap against M006 or M008 and a redacted availability report.

## Must-Haves

- M006 30-paper IDs and M008 10-paper IDs are excluded.
- Candidate inventory is redacted and deterministic.
- Selected count is 10.
- Prior overlap count is 0.
- Availability report records Markdown/PDF/workspace counts.
- No raw paper text is included.

## Proof Level

- This slice proves: Manifest and overlap guard checks.

## Integration Closure

Produces the manifest consumed by S02 init/preflight/top-up.

## Verification

- Adds selected manifest, candidate inventory, overlap report, and availability report without raw paper text.

## Tasks

- [x] **T01: Build prior exclusion and candidate inventory** `est:small`
  Build prior-corpus exclusion set from M006 and M008 manifests and a redacted candidate inventory from local/cache paper metadata. Do not include raw text.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json && uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json')
s=json.loads(p.read_text())
assert s['candidate_count'] >= 10
assert s['raw_text_included'] is False
print('candidate-inventory-ok')
PY

- [x] **T02: Select next plus ten manifest** `est:small`
  Select the first 10 deterministic candidate IDs after exclusions and write the M010 manifest plus rationale.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json`, `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-selection-rationale.md`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json').read_text())
assert s['paper_count']==10
assert s['prior_overlap_count']==0
assert s['raw_text_included'] is False
print('manifest-ok')
PY

- [x] **T03: Write availability report and selection guard** `est:small`
  Write availability report and run final S01 guards: selected count, overlap count, source availability counts, and safety flags.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-availability-report.md`, `.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json').read_text())
assert g['selected_count']==10
assert g['prior_overlap_count']==0
assert g['raw_text_included'] is False
print('selection-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json
- .gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json
- .gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-selection-rationale.md
- .gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-availability-report.md
- .gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json
