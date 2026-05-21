---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Map ML package references

Search repo source/tests/docs for direct references to torch, torchvision, transformers, accelerate, docling, and conversion paths. Produce a sanitized reachability JSON artifact with file:line references and package ownership classification.

## Inputs

- `src/`
- `tests/`
- `doc/`
- `pyproject.toml`
- `uv.lock`

## Expected Output

- `.gsd/milestones/M018-gyff0h/slices/S02/run-evidence/ml-reachability-map.json`

## Verification

uv run python inline assertions over ml-reachability-map.json

## Observability Impact

Records static reachability evidence for future agents.
