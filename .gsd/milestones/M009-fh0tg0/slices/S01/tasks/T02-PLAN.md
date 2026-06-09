---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Added provenance/freshness unit tests covering redaction, hash matching, stale/missing outputs, and unsafe flags.

Add unit tests covering fingerprint redaction, provenance entry creation, append/read JSONL, freshness pass, output mutation stale failure, missing output failure, unsafe safety flags, and entry selection.

## Inputs

- `src/arxiv_archive/validation_batch_provenance.py`

## Expected Output

- `tests/test_validation_batch_provenance.py`

## Verification

uv run pytest tests/test_validation_batch_provenance.py -q && uv run ruff check src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py

## Observability Impact

Tests prove logs do not embed raw content and stale artifacts are detectable.
