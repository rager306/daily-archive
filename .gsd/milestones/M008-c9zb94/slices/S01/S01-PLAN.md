# S01: S01

**Goal:** Select exactly 10 new paper IDs for the first real expansion batch, excluding all M006 corpus papers, and document the deterministic selection rule plus source availability preview.
**Demo:** After this slice, there is a deterministic next-10 manifest that excludes all M006 papers and explains why each paper was selected.

## Must-Haves

- Exactly 10 unique paper IDs selected.
- No selected paper overlaps M006 30-paper corpus.
- Deterministic selection rule is documented.
- Source availability preview includes paths/status only, no raw paper text.
- No acquisition, scan, or KG import occurs.

## Proof Level

- This slice proves: Manifest and guard checks for uniqueness/no-overlap/source-preview redaction.

## Integration Closure

Consumes M006 30-paper manifest and local research/cache inventory; produces M008 +10 manifest for S02 validation-batch init/preflight.

## Verification

- Adds new +10 manifest, overlap audit, and source availability preview artifacts without raw text.

## Tasks

- [x] **T01: Built a redacted candidate inventory with 800 non-M006 candidates.** `est:small`
  Survey existing local research/cache inventory (`/root/.research/papers` and `/root/.arxiv_cache`) plus the M006 manifest to identify candidate paper IDs not already in the 30-paper corpus. Produce a compact candidate inventory summary without raw text.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json && uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json')
s=json.loads(p.read_text())
assert s['candidate_count'] >= 10
assert s['raw_text_included'] is False
print('candidate-inventory-ok')
PY

- [x] **T02: Selected the deterministic first new +10 corpus manifest.** `est:small`
  Apply the deterministic selection rule to choose exactly 10 new paper IDs and write the M008 manifest plus rationale. The manifest should be compatible with validation-batch init.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`, `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-selection-rationale.md`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json && uv run python - <<'PY'
import json
from pathlib import Path
m=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json').read_text())
ids=[p['paper_id'] for p in m['papers']]
assert len(ids)==10
assert len(set(ids))==10
assert m['raw_text_included'] is False
print('new-plus-ten-manifest-ok')
PY

- [x] **T03: Audited the new +10 manifest: no M006 overlap, 1/10 Markdown-ready before S02.** `est:small`
  Run an overlap/source preview guard against M006 corpus and write a short availability report. Confirm no overlap and no raw text leakage.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-availability-report.md`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-availability-report.md && uv run python - <<'PY'
import json
from pathlib import Path
m=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json').read_text())
old=json.loads(Path('.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json').read_text())
ids={p['paper_id'] for p in m['papers']}
old_ids={p['paper_id'] for p in old['papers']}
assert not ids & old_ids
print('overlap-audit-ok')
PY

## Files Likely Touched

- .gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json
- .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json
- .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-selection-rationale.md
- .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-availability-report.md
