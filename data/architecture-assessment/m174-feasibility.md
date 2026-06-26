# M174 Feasibility

## Verdict

**Proceed with repair benchmark category expansion.**

The category is feasible because the candidates are clustered under two exact repair infrastructure modules and there is one existing exception (`caller-owned-index`) that can be preserved by rule order and tests.

## Why proceed

- Baseline inventory is green: `unknown=0`.
- Candidate set is small: 5 movable records plus 1 preserved exception.
- Exact source paths are stable repair benchmark modules.
- Existing policy D094/D095 already requires exact path-family matching and fallback tests.

## Safety rules

1. Add only exact source path rules.
2. Preserve `caller-owned-index` for `chunk_baseline_measurement.py` + `index_path`.
3. Do not classify by target words such as `diagnostics`, `summary`, `review`, or `benchmark` globally.
4. Add a positive repair benchmark test and a preserved index exception test.
5. Add a fallback test for similar unapproved repair-like paths.
