# S02: S02

**Goal:** Map torch/transformers imports and execution reachability to CLI/runtime/test/probe paths.
**Demo:** After S02, repo import/use sites identify whether vulnerable ML packages are active runtime, dev/test, probe-only, or unused.

## Must-Haves

- Import/use sites for torch, transformers, and dependent ML adapters are listed with file references.
- Each site is classified by runtime exposure.
- Untrusted input boundaries are identified if present.
- No code changes are made unless documentation artifacts are written.

## Proof Level

- This slice proves: Static import search, targeted code inspection, and guard assertions.

## Integration Closure

Reachability classification feeds S03 risk decision.

## Verification

- Creates a file:line reachability report for future agents.

## Tasks

- [x] **T01: Mapped ML package references and classified direct torch/transformers source imports as absent.** `est:45m`
  Search repo source/tests/docs for direct references to torch, torchvision, transformers, accelerate, docling, and conversion paths. Produce a sanitized reachability JSON artifact with file:line references and package ownership classification.
  - Files: `.gsd/milestones/M018-gyff0h/slices/S02/run-evidence/ml-reachability-map.json`
  - Verify: uv run python inline assertions over ml-reachability-map.json

- [x] **T02: Classified runtime exposure as medium when bounded source acquisition processes external PDFs, otherwise low/dormant.** `est:60m`
  Inspect the active conversion/runtime files identified by T01 and classify whether vulnerable ML packages are reachable from CLI/runtime paths that process untrusted PDFs or other external inputs. Write a human-readable reachability report.
  - Files: `src/arxiv_archive/md_converter.py`, `src/arxiv_archive/full_text.py`, `src/arxiv_archive/pdf_downloader.py`, `.gsd/milestones/M018-gyff0h/slices/S02/ml-reachability-report.md`
  - Verify: uv run python inline assertions over ml-reachability-map.json and report existence

## Files Likely Touched

- .gsd/milestones/M018-gyff0h/slices/S02/run-evidence/ml-reachability-map.json
- src/arxiv_archive/md_converter.py
- src/arxiv_archive/full_text.py
- src/arxiv_archive/pdf_downloader.py
- .gsd/milestones/M018-gyff0h/slices/S02/ml-reachability-report.md
