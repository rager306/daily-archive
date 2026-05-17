---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Verify artifact to ingestion boundary for PageIndex consumers

Wire the ingestion boundary to existing stored paper artifact assumptions without changing the public daily CLI. Add tests proving a stored paper id plus deterministic local source path can produce an ingestion result ready for PageIndex construction, and document the S01 boundary in module docstrings or test names. Run targeted tests plus lint on the new production module. Done when future S02 can consume the result shape without touching M001 cron artifacts.

## Inputs

- `src/arxiv_archive/full_text.py`
- `tests/test_full_text_ingestion.py`
- `src/arxiv_archive/cli.py`

## Expected Output

- `src/arxiv_archive/full_text.py`
- `tests/test_full_text_ingestion.py`

## Verification

uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py -q

## Observability Impact

Confirms ingestion diagnostics are preserved at the boundary that will feed PageIndex construction: paper id, source path, extraction mode, warnings, fallback reason, and provenance.
