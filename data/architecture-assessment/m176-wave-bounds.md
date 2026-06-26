# M176 Wave Bounds

## Verdict

**Wave one status: bounded.** M176 will reduce `script-only` only for reviewed exact script families. It will not attempt to classify all 265 script records.

## Target size

Wave one should move roughly 10 to 30 script records if exact families are safe. This keeps review small enough to verify with focused tests and generated delta.

## Candidate priority

1. Highest-count coherent milestone families, especially `m061_*` and `m058_*` if source review confirms shared output ownership.
2. Repeated audit or build families that are stable and exact.
3. Exclude mixed one-off scripts unless a path-family is obvious.

## Stop conditions

Stop and leave records as `script-only` when any condition is true:

- only target names connect the records;
- script purpose is unclear from local source review;
- script writes shared state, cache-like paths, queue state, or canonical data;
- category would mix unrelated milestones;
- tests would need broad fixture setup beyond `_classify` checks.

## Required proof for every moved family

- exact file path list;
- category name;
- positive `_classify` test;
- fallback test showing an unrelated script remains `script-only`;
- generated final inventory and delta.

## Expected residual

`script-only` should remain large after M176. That is acceptable. The objective is crystallization by safe increments, not hiding script debt behind broad labels.
