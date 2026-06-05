---
id: T02
parent: S06
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl
key_decisions:
  - Future parser-quality evaluation must run graph-readiness review post-check before manifest synthesis and must preserve current low_quality_source/no_substantive_body refusal behavior.
duration: 
verification_result: passed
completed_at: 2026-06-05T11:49:04.044Z
blocker_discovered: false
---

# T02: Specified measurable quality metrics and acceptance gates for the future combined-parser probe.

**Specified measurable quality metrics and acceptance gates for the future combined-parser probe.**

## What Happened

Created `quality-metrics-and-gates.json` and `.md`. The plan defines seven quality dimensions: GROBID TEI/bibliography/citation quality, OpenDataLoader layout/OCR/table/coordinate quality, Adaptix adapter contract coverage, tree/PageIndex/card/provenance schema fit, source-span anchoring and staleness, low-quality/refusal preservation, and review-packet/graph-readiness boundaries. It also records global acceptance rules: parser output is candidate evidence only, no production integration, no graph import or LadybugDB write, typed diagnostics for failures, no secrets/raw article bodies in logs, and graph-readiness review post-check before future manifest synthesis.

## Verification

Fresh T02 verification passed in `gsd_exec[3cb4bab6-a219-40e5-bd51-ab57f02e9b00]`: the script validated all seven expected metric categories, graph-readiness review post-check expectations, S06 future scope not executed in M033, and all false safety flags. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 metrics/gates-generation validation script via gsd_exec purpose 'M033 S06 T02 create quality metrics and gates'` | 0 | ✅ pass | 113ms |

## Deviations

None.

## Known Issues

The metric thresholds are specified as future gates and categories; a future milestone must bind them to concrete paper IDs and exact numeric pass/fail thresholds after corpus selection.

## Files Created/Modified

- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl`
