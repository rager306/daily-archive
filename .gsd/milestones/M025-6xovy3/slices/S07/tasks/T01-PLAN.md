---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Defined metadata-safe separated evidence boundary fixtures for assets, tables, links, and identity with executable contract tests.

Define the separated metadata-safe evidence artifact contracts for assets, tables, links, and identity. The contracts must reference article/source/element/chunk identifiers without embedding raw payload text or binary data, and must keep graph import and production write flags false. At execution time this task consumes S01 catalog/index/selection outputs and S06 chunking outputs, but those future artifacts are intentionally not listed as static inputs for pre-execution validation.

## Inputs

- `.gsd/milestones/M025-6xovy3/M025-6xovy3-ROADMAP.md`

## Expected Output

- `tests/fixtures/article_evidence_boundaries_v00_01/`
- `tests/test_article_evidence_boundaries.py`

## Verification

uv run pytest tests/test_article_evidence_boundaries.py -q
uv run ruff check tests/test_article_evidence_boundaries.py

## Observability Impact

Adds executable checks for metadata-safe evidence artifacts and no graph-import flags before implementation code writes them.
