---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T03: Define artifact contracts, diagnostics, and failure taxonomy

Specify the artifact tree, JSON schemas or schema-shape expectations, diagnostic event taxonomy, no-secret/no-raw-text logging rule, typed blocker states, no-write import rehearsal expectations, rollback conditions, and adoption-decision thresholds for the future milestone.

## Inputs

- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json`

## Expected Output

- `data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.json`
- `data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.md`
- `data/article_corpora/m033-external-parser-quality-plan-v1/adoption-and-rollback-criteria.md`

## Verification

Fresh command validates artifact/diagnostic contracts exist and include no raw text/secrets, typed blockers, no-write import rehearsal, rollback, and false safety flags.

## Observability Impact

Ensures future agents know what evidence to produce and what failures mean.
