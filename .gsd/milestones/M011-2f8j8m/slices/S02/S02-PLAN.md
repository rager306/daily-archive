# S02: Semantic rubric and redacted judgments

**Goal:** Define and apply a semantic review rubric that judges whether selected M010 targets are extraction-ready, retrieval-only, repair-required, or rejected without embedding raw source text.
**Demo:** A review rubric and redacted judgment packet are available for the selected targets, with decisions expressed as categories and source references rather than raw text.

## Must-Haves

- Rubric separates chunk boundary quality, claim supportability, provenance adequacy, and import blockers.
- Redacted judgments cover every selected target.
- Judgments do not create trusted KG facts or include raw claim text.
- Production import and LadybugDB writes remain false.

## Proof Level

- This slice proves: Rubric consistency checks and redaction guard.

## Integration Closure

Consumes S01 review targets and produces redacted per-target judgments for S03 independent review.

## Verification

- Adds rubric, judgment summary, issue taxonomy, and no-write guard.

## Tasks

- [x] **T01: Define semantic import-readiness rubric** `est:small`
  Write a semantic import-readiness rubric that can classify targets as import_candidate, retrieval_only, repair_required, or reject, with explicit blockers for missing chunk spans and no trusted claim text.
  - Files: `.gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md`
  - Verify: test -s .gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md

- [x] **T02: Apply redacted semantic judgments** `est:medium`
  Apply the rubric to every S01 target using redacted M010 metadata and source path/hash provenance. Persist categorical judgments without raw source text or claim text.
  - Files: `.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json`, `.gsd/milestones/M011-2f8j8m/slices/S02/semantic-judgment-summary.md`
  - Verify: test -s .gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json && uv run python - <<'PY'
import json
from pathlib import Path
j=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json').read_text())
assert j['target_count'] == 10
assert j['raw_text_included'] is False
assert j['trusted_facts_created'] is False
print('redacted-semantic-judgments-ok')
PY

- [x] **T03: Verify judgment consistency and leakage guard** `est:small`
  Run consistency and leakage guard over the rubric and judgments, including class counts, blocker counts, and no-write/no-import safety flags.
  - Files: `.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/semantic-judgment-guard.json`
  - Verify: test -s .gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/semantic-judgment-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/semantic-judgment-guard.json').read_text())
assert g['target_count'] == 10
assert g['raw_payload_key_count'] == 0
assert g['positive_import_recommended'] is False
print('semantic-judgment-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md
- .gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json
- .gsd/milestones/M011-2f8j8m/slices/S02/semantic-judgment-summary.md
- .gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/semantic-judgment-guard.json
