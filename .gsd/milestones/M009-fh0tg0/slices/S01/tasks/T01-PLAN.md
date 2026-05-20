---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Implement provenance primitives

Create validation_batch_provenance module with file fingerprinting, argv redaction, git commit lookup, provenance entry construction, JSONL append/read, entry selection, and freshness report generation/writing. Keep it independent of CLI wiring.

## Inputs

- `src/arxiv_archive/validation_batch_state.py`

## Expected Output

- `src/arxiv_archive/validation_batch_provenance.py`

## Verification

uv run python - <<'PY'
from pathlib import Path
from arxiv_archive.validation_batch_provenance import fingerprint_file, redact_cli_args
p=Path('/tmp/provenance-smoke.txt')
p.write_text('secret sentinel text')
f=fingerprint_file(p)
assert f['sha256'].startswith('sha256:')
assert 'sentinel' not in str(f)
assert 'abc' not in str(redact_cli_args(['cmd','--api-key','abc']))
print('provenance-primitives-ok')
PY

## Observability Impact

Adds provenance/freshness primitives with redacted safety flags.
