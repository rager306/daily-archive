---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T02: Classified runtime exposure as medium when bounded source acquisition processes external PDFs, otherwise low/dormant.

Inspect the active conversion/runtime files identified by T01 and classify whether vulnerable ML packages are reachable from CLI/runtime paths that process untrusted PDFs or other external inputs. Write a human-readable reachability report.

## Inputs

- `.gsd/milestones/M018-gyff0h/slices/S02/run-evidence/ml-reachability-map.json`
- `targeted code inspection`

## Expected Output

- `.gsd/milestones/M018-gyff0h/slices/S02/ml-reachability-report.md`

## Verification

uv run python inline assertions over ml-reachability-map.json and report existence

## Observability Impact

Documents active vs dormant exposure and untrusted-input boundary.
