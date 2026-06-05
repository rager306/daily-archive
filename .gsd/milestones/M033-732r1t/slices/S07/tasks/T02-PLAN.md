---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Add focused tests for adapter mapping and safety flags

Add tests that exercise Adaptix loading against a small fixture with OpenDataLoader field names, verify aliases and bounding boxes map correctly, verify malformed documents fail closed with diagnostics, and verify generated summaries keep graph/import/LadybugDB flags false. Keep tests local-only and independent of the hybrid backend.

## Inputs

- `scripts/probe_m033_opendataloader_adaptix_adapter.py`

## Expected Output

- `tests/test_m033_opendataloader_adaptix_adapter.py`

## Verification

uv run pytest tests/test_m033_opendataloader_adaptix_adapter.py -q

## Observability Impact

Adds regression coverage for the adapter boundary and safety flag invariants.
