---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Define bounded source acquisition plan

Inspect existing full-text/download/conversion code paths and define a bounded acquisition plan for the 20 missing-Markdown papers. The plan should prefer already-supported project mechanisms and avoid unbounded Marker/LLM/optimizer behavior.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json`
- `src/arxiv_archive/full_text.py`
- `src/arxiv_archive/md_converter.py`
- `src/arxiv_archive/pdf_downloader.py`

## Expected Output

- `.gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md`

## Verification

test -s .gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md && grep -q 'bounded' .gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md

## Observability Impact

Plan records method order and operational limits before running acquisition.
