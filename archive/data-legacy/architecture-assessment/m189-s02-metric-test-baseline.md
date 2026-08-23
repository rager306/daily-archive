# M189 S02 Metric Test Baseline

## Verdict

**PASS: existing extraction and non-ablation evaluation metric tests are green.**

## Evidence

| Test scope | Result | Evidence |
|---|---|---|
| Extraction benchmark tests | PASS: 6 passed | `gsd_exec[a32fc90c-9e3f-4ecf-925e-362067887c34]` |
| Non-ablation evaluation metric tests | PASS: 6 passed, 2 deselected | `gsd_exec[e10048fc-c314-4e02-90c5-4cdca30256d4]` |

## Metric surfaces covered

- extraction benchmark fixture schema validity;
- reviewed fixture extraction metrics;
- perfect-record scoring;
- invalid prediction schema handling;
- strict gold fixture validation;
- queue payload metric storage;
- groundedness proxy evidence IDs;
- evidence path hit rate;
- retrieval recall handling for duplicates, missing IDs, `None`, and empty lists.

## Interpretation

These tests prove existing metric primitives and fixtures are internally consistent. They do not prove production real-corpus quality, graph import readiness, DSPy optimization readiness, or RLM production retrieval quality.
