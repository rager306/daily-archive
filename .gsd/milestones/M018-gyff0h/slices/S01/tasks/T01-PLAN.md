---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T01: Captured sanitized ML dependency inventory and transitive path from docling to torch/transformers.

Inspect Python dependency declarations and lockfile summaries for torch/transformers ownership. Run sanitized package inventory commands using uv without modifying dependencies. Write a dependency inventory artifact under S01 run-evidence.

## Inputs

- `pyproject.toml`
- `uv.lock`
- `uv package inventory command output`

## Expected Output

- `.gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-inventory.json`
- `.gsd/milestones/M018-gyff0h/slices/S01/dependency-inventory-report.md`

## Verification

uv run python scripts/guard-style JSON assertions or equivalent inline assertion over dependency-inventory.json

## Observability Impact

Captures exact commands, exit codes, and sanitized dependency facts for reproducibility.
