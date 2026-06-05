---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Inventory current parser pipeline entrypoints

Use GitNexus and repository evidence to identify the current daily-archive scripts/modules/tests that participate in catalog intake, source acquisition, loader evidence, parser/conversion, chunk/evidence replay, graph-readiness package generation, and no-write import refusal. Record file paths, symbol/process names where available, stage ownership, and what each entrypoint produces. Do not edit code.

## Inputs

- `.gsd/PROJECT.md`
- `.gsd/REQUIREMENTS.md`

## Expected Output

- `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.json`
- `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.md`

## Verification

Manual review — file exists and is non-empty

## Observability Impact

Entrypoint inventory gives future agents exact code/artifact surfaces to inspect before comparing external tools.
