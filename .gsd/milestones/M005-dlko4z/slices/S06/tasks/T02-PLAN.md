---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Implemented deterministic benchmark adapters for baseline, structure-aware, and simple section-window estimate methods.

Implement deterministic benchmark adapters for existing S02 baseline evidence, S03/S04/S05 structure-aware evidence, and one bounded candidate that uses preserved normalized Markdown/source spans to estimate simple section-window chunking diagnostics. Do not add heavy dependencies or execute Chonkie/LlamaIndex/LangChain yet; record them as later benchmark candidates unless explicitly installed and bounded.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl`

## Expected Output

- `src/arxiv_archive/chunking_benchmark.py`
- `tests/test_chunking_benchmark.py`

## Verification

uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunking_benchmark.py tests/test_chunking_benchmark.py

## Observability Impact

Adapters should emit method-specific caveats and explicit unsupported-candidate notes for real libraries not executed in this slice.
