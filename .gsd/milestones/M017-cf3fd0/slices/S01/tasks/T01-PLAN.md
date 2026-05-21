---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Synthesize Manus MiniMax research

Attempt Manus share extraction with Jina read/json/html modes for https://manus.im/share/TSUZT2btrNfwnq5TXXDQm9, summarize accessible content or document access limitation, and map implications to M017 design.

## Inputs

- None specified.

## Expected Output

- `.gsd/milestones/M017-cf3fd0/slices/S01/run-evidence/manus-jina-extraction-summary.json`
- `.gsd/milestones/M017-cf3fd0/slices/S01/manus-minimax-research-synthesis.md`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M017-cf3fd0/slices/S01/run-evidence/manus-jina-extraction-summary.json').read_text())
assert s['url'].startswith('https://manus.im/share/')
assert s['jina_attempted'] is True
assert 'design_implications' in s
print('manus-jina-synthesis-ok')
PY

## Observability Impact

Records whether the external research was actually extractable and how it affects helper design.
