# M141 DSPy Boundary Repair

Schema: `daily-archive-m141-dspy-repair.v1`

Corrected `S08_FILES` source path from dotted filename to real package path:

- Before: `src/research_graph.infrastructure.evaluation.dspy_extraction.py`
- After: `src/research_graph/infrastructure/evaluation/dspy_extraction.py`

## Verification

- Focused pytest: `9 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
