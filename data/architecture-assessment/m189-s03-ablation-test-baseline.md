# M189 S03 Ablation Test Baseline

## Verdict

**PASS: retrieval ablation and DSPy boundary gates are green without enabling optimizer work.**

## Evidence

| Test scope | Result | Evidence |
|---|---|---|
| Retrieval ablation tests | PASS: 2 passed, 6 deselected | `gsd_exec[c4757d35-c8e3-4925-afa3-4a8747f6e98d]` |
| DSPy boundary tests | PASS: 9 passed | `gsd_exec[20c3dd2a-1eed-4186-896d-52c089cb42ee]` |

## Interpretation

- Retrieval ablation runner behavior is covered for fixture modes and missing/empty result diagnostics.
- DSPy remains a boundary surface only; tests passing does not authorize optimizer activation.
- Hybrid retrieval remains deterministic fixture-level baseline, not production retrieval quality.
- Graph import, LadybugDB writes, and production persistence remain out of scope.
