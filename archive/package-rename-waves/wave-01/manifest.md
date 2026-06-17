# Package Rename Wave 01 Manifest

Milestone: M089-47q2c5

Purpose: move the first implementation-bearing modules from `arxiv_archive` into `research_graph` after the M088 skeleton.

## Moves

| Old path | New canonical path | Archive path | Status |
|---|---|---|---|
| `src/arxiv_archive/artifacts/metrics.py` | `src/research_graph/papers/artifacts/metrics.py` | `archive/package-rename-waves/wave-01/src/arxiv_archive/artifacts/metrics.py` | archived implementation |
| `src/arxiv_archive/artifacts/minimax_boundary.py` | `src/research_graph/papers/artifacts/minimax_boundary.py` | `archive/package-rename-waves/wave-01/src/arxiv_archive/artifacts/minimax_boundary.py` | archived implementation |
| `src/arxiv_archive/artifacts/reducer.py` | `src/research_graph/papers/artifacts/reducer.py` | `archive/package-rename-waves/wave-01/src/arxiv_archive/artifacts/reducer.py` | archived implementation |
| `src/arxiv_archive/artifacts/assets.py` | `src/research_graph/papers/assets.py` | `archive/package-rename-waves/wave-01/src/arxiv_archive/artifacts/assets.py` | archived implementation |
| `src/arxiv_archive/artifacts/evidence_bridge.py` | `src/research_graph/papers/evidence.py` | `archive/package-rename-waves/wave-01/src/arxiv_archive/artifacts/evidence_bridge.py` | archived implementation |
| `src/arxiv_archive/llm/provider_config.py` | `src/research_graph/llm/provider_config.py` | `archive/package-rename-waves/wave-01/src/arxiv_archive/llm/provider_config.py` | archived implementation |

## Breadcrumb rule

Each new canonical module contains a `Formerly: src/arxiv_archive/...` breadcrumb.

## Intentional breakage

The moved `arxiv_archive.artifacts.*` and `arxiv_archive.llm.provider_config` import paths are no longer runtime canonical paths after this wave. Internal code and tests should import from `research_graph.*`.

## Verification contract

- direct old import search for moved module paths is clean in `src`, `tests`, and `scripts`;
- targeted artifact/LLM/worker/e2e/CLI/scaffold tests pass;
- `python3 -m py_compile` passes for moved modules and affected importers;
- import smoke for `research_graph` wave-01 modules passes;
- GitNexus detect_changes is reviewed.
