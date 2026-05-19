---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Select thirty paper deviation corpus

Discover candidate paper ids from local artifacts, caches, and M005/M004 manifests. Also inspect local external artifact roots `/root/.research/papers` and `/root/.arxiv_cache` during execution, but record only redacted paths/status in outputs. Select 30 unique ids deterministically, preserving the M005 10-paper baseline overlap and adding 20 expansion papers from available local evidence. Record selection rationale and risk tags.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `.gsd/milestones/M004-ubh2pt/slices/S01/ten-doc-corpus.json`

## Expected Output

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-rationale.md`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
manifest=json.loads(Path('.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json').read_text())
ids=[p['paper_id'] for p in manifest['papers']]
assert len(ids)==30
assert len(set(ids))==30
assert manifest['production_import_attempted'] is False
assert manifest['ladybugdb_written'] is False
assert manifest['raw_text_included'] is False
print({'paper_count': len(ids), 'm005_overlap_count': manifest['m005_overlap_count']})
PY

## Observability Impact

Manifest records why each paper is included and which source/risk tags apply.
