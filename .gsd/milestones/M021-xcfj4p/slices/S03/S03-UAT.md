# S03: Bounded batch implementation rehearsal — UAT

**Milestone:** M021-xcfj4p
**Written:** 2026-05-21T10:32:10.123Z

# S03 UAT

## Scenario

A reviewer needs deterministic candidate locator output over the bounded M011 batch.

## Expected behavior

- The deterministic batch artifact exists.
- It covers 10 papers and 26 locators.
- It reports ambiguity diagnostics and safety flags.
- It has zero import-eligible locators and zero fact promotions.
- It validates with the candidate locator module.
- It is more route-filtered than M020's hand-built artifact but still not a positive import signal.

## Evidence

```text
uv run pytest tests/test_candidate_locators.py -q
10 passed

uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py
All checks passed!

m021-s03-deterministic-batch-guard-ok
m021-s03-final-verification-ok
```

## Verdict

PASS for deterministic batch rehearsal. Proceed to S04 independent review; do not proceed to positive import.

