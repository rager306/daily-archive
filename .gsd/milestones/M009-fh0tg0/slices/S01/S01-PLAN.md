# S01: CLI run provenance log

**Goal:** Add a safe, additive provenance/freshness module that can record validation CLI run metadata and verify recorded file hashes without changing existing CLI behavior yet.
**Demo:** After this slice, a validation-batch run can emit a commit-safe provenance log entry tying command execution to input and output hashes.

## Must-Haves

- New provenance module hashes files without serializing contents.
- CLI args redaction removes secret-like flag values.
- Provenance entry captures command, argv, cwd, git commit, timestamps, exit code, input/output hashes, and safety flags.
- JSONL append/read round-trips entries.
- Freshness report passes unchanged files and fails changed/missing/unsafe provenance.
- Existing validation-batch workflow tests remain unaffected.

## Proof Level

- This slice proves: Unit tests for hashing, redaction, provenance JSONL, and freshness pass/fail cases.

## Integration Closure

Provides library primitives for S02 CLI verifier and later command integration.

## Verification

- Adds command/run metadata, input/output hashes, safety flags, and stale artifact diagnostics without raw text content.

## Tasks

- [x] **T01: Implement provenance primitives** `est:medium`
  Create validation_batch_provenance module with file fingerprinting, argv redaction, git commit lookup, provenance entry construction, JSONL append/read, entry selection, and freshness report generation/writing. Keep it independent of CLI wiring.
  - Files: `src/arxiv_archive/validation_batch_provenance.py`
  - Verify: uv run python - <<'PY'
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

- [x] **T02: Test provenance and freshness behavior** `est:medium`
  Add unit tests covering fingerprint redaction, provenance entry creation, append/read JSONL, freshness pass, output mutation stale failure, missing output failure, unsafe safety flags, and entry selection.
  - Files: `tests/test_validation_batch_provenance.py`
  - Verify: uv run pytest tests/test_validation_batch_provenance.py -q && uv run ruff check src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py

- [x] **T03: Run regression and sample provenance artifacts** `est:small`
  Run regression checks to ensure new module does not alter existing validation-batch workflow behavior, then write S01 sample run-log/freshness artifacts for review.
  - Files: `.gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl`, `.gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json`
  - Verify: uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_workflow.py -q && uv run ruff check src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py && test -s .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl && test -s .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json

## Files Likely Touched

- src/arxiv_archive/validation_batch_provenance.py
- tests/test_validation_batch_provenance.py
- .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl
- .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json
