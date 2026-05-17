---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T04: Run S03 quality gates and regression smoke

Run final S03 regression gates: evidence-path tests, PageIndex tests, S01 ingestion tests, analysis regression, CLI contract smoke, Ruff on touched files, and public module help smoke. Record limitations for S04 and S05: no claims/entities, no embeddings, no LadybugDB persistence, and simple deterministic chunking only. Done when S03 is ready for closeout and requirements restoration can follow.

## Inputs

- `src/arxiv_archive/evidence.py`
- `tests/test_evidence_paths.py`
- `src/arxiv_archive/page_index.py`
- `src/arxiv_archive/full_text.py`
- `src/arxiv_archive/cli.py`
- `tests/test_cli_contract.py`

## Expected Output

- `src/arxiv_archive/evidence.py`
- `tests/test_evidence_paths.py`

## Verification

uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Observability Impact

Final gates confirm chunk/evidence diagnostics remain available and public CLI behavior remains unchanged.
