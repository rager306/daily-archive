---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Defined the bounded future parser-quality probe scope and corpus strategy.

After S05 is complete, create the future quality milestone scope from the S05 recommendation: selected corpus classes, golden/fixture paper types, required source-locality controls, no-network defaults, model/backend cache checks, and excluded production actions.

## Inputs

- `.gsd/milestones/M033-732r1t/slices/S05/S05-PLAN.md`

## Expected Output

- `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json`
- `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl`

## Verification

Fresh command validates scope artifacts exist, include corpus classes and excluded production actions, and keep safety flags false.

## Observability Impact

Frames a future milestone in bounded, local, fail-closed terms.
