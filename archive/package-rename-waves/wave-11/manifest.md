# Package Rename Wave 11 Manifest

Scope: quality diagnostics package.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/quality/__init__.py` | `src/research_graph/infrastructure/quality/__init__.py` | `archive/package-rename-waves/wave-11/src/arxiv_archive/quality/__init__.py` |
| `src/arxiv_archive/quality/baselines.py` | `src/research_graph/infrastructure/quality/baselines.py` | `archive/package-rename-waves/wave-11/src/arxiv_archive/quality/baselines.py` |
| `src/arxiv_archive/quality/maintainability_report.py` | `src/research_graph/infrastructure/quality/maintainability_report.py` | `archive/package-rename-waves/wave-11/src/arxiv_archive/quality/maintainability_report.py` |
| `src/arxiv_archive/quality/riskratchet_adapter.py` | `src/research_graph/infrastructure/quality/riskratchet_adapter.py` | `archive/package-rename-waves/wave-11/src/arxiv_archive/quality/riskratchet_adapter.py` |
| `src/arxiv_archive/quality/scopes.py` | `src/research_graph/infrastructure/quality/scopes.py` | `archive/package-rename-waves/wave-11/src/arxiv_archive/quality/scopes.py` |
| `src/arxiv_archive/quality/thresholds.py` | `src/research_graph/infrastructure/quality/thresholds.py` | `archive/package-rename-waves/wave-11/src/arxiv_archive/quality/thresholds.py` |

## Verification Notes

- Quality diagnostic helpers moved to `research_graph.quality`.
- No compatibility shim is retained under `src/arxiv_archive/quality`.
- Tests remain local-only and non-blocking; riskratchet absence remains a diagnostic state, not a hard failure.
