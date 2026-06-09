# S01: S01

**Goal:** Select bounded semantic review targets from M010 chunks/outliers without embedding raw paper or chunk text in artifacts.
**Demo:** A redacted semantic review corpus manifest exists, pointing to source files by path/hash/span and M010 scan evidence by artifact path, with leakage guard passing.

## Must-Haves

- Review set includes a bounded mix of outliers and non-outlier controls from M010.
- Each target has paper id, source path, source hash, chunk/span identifiers where available, and reason for inclusion.
- No raw paper text, chunk text, embeddings, vectors, secrets, or optimizer traces are embedded.
- Selection is deterministic and reproducible from M010 artifacts.

## Proof Level

- This slice proves: Artifact guard plus explicit no-raw-text assertions.

## Integration Closure

Consumes M010 S02 source-ready batch and S03 scan/outlier artifacts; provides a review-set manifest for S02.

## Verification

- Adds redacted target manifest, selection rationale, and leakage guard.

## Tasks

- [x] **T01: Inspected M010 reviewable metadata and recorded a no-payload schema summary for S01 selection.** `est:small`
  Inspect M010 scan/outlier artifact schemas and identify which metadata fields can support redacted semantic review selection without raw text.
  - Files: `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/schema-inspection.json`
  - Verify: test -s .gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/schema-inspection.json

- [x] **T02: Built the M011 semantic review target set: 10 redacted targets with source paths and hashes.** `est:medium`
  Build deterministic redacted review-set manifest with a bounded mix of M010 outliers and non-outlier controls, carrying source path/hash/span metadata only.
  - Files: `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json`, `.gsd/milestones/M011-2f8j8m/slices/S01/semantic-review-selection-rationale.md`
  - Verify: test -s .gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json && uv run python - <<'PY'
import json
from pathlib import Path
m=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json').read_text())
assert m['target_count'] > 0
assert m['raw_text_included'] is False
assert m['chunk_text_included'] is False
print('semantic-review-targets-ok')
PY

- [x] **T03: Verified the M011 S01 review set guard: 10 targets, 7 outliers, 3 controls, no raw payload keys.** `est:small`
  Run leakage and reproducibility guard over S01 artifacts, then write final selection guard.
  - Files: `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/selection-guard.json`
  - Verify: test -s .gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/selection-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/selection-guard.json').read_text())
assert g['target_count'] > 0
assert g['safety_flags_false'] is True
assert g['raw_payload_key_count'] == 0
print('semantic-selection-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/schema-inspection.json
- .gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json
- .gsd/milestones/M011-2f8j8m/slices/S01/semantic-review-selection-rationale.md
- .gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/selection-guard.json
