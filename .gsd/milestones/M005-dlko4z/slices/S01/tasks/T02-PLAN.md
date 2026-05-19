---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Select representative gold corpus

Select the representative gold corpus for chunking/import benchmarks from existing real-paper artifacts. Include target paper IDs, why each paper is selected, expected hard cases, and required artifact paths. Keep this as a manifest, not a broad corpus run.

## Inputs

- `.gsd/milestones/M004-ubh2pt/slices/S01/ten-doc-corpus.json`
- `.gsd/milestones/M004-ubh2pt/slices/S07/run-evidence/retrieval-fixture-load.json`
- `.gsd/milestones/M004-ubh2pt/slices/S10/chunk-review/2605.14517v1-chunk-sample.md`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-rationale.md`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json')
data=json.loads(p.read_text())
assert data['schema_version']=='m005-gold-corpus-manifest.v1'
assert len(data['papers']) >= 6
assert all('paper_id' in paper and 'hard_case_tags' in paper for paper in data['papers'])
assert data['broad_corpus_run'] is False
PY

## Observability Impact

Manifest records corpus scope, hard-case tags, artifact availability, and explicit non-scaling boundary.
