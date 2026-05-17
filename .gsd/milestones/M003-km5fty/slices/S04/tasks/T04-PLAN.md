---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T04: Run S04 quality gates and regression smoke

Run final S04 regression gates: extraction contract tests, S03 evidence tests, PageIndex tests, S01 ingestion tests, analysis regression, CLI contract smoke, Ruff on touched files, and public module help smoke. Record limitations for S05/S07: contracts are deterministic drafts only, no extraction model, no embeddings, no LadybugDB persistence, no DSPy/RLM. Done when S04 is ready for closeout.

## Inputs

- `src/arxiv_archive/scientific_extraction.py`
- `tests/test_scientific_extraction_contracts.py`
- `src/arxiv_archive/evidence.py`
- `src/arxiv_archive/page_index.py`
- `src/arxiv_archive/full_text.py`
- `src/arxiv_archive/cli.py`
- `tests/test_cli_contract.py`

## Expected Output

- `src/arxiv_archive/scientific_extraction.py`
- `tests/test_scientific_extraction_contracts.py`

## Verification

uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Observability Impact

Final gates confirm extraction contract diagnostics remain available and public CLI behavior remains unchanged.
