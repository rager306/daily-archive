# Package Layout Shim Archive Wave 01

Purpose: remove already-migrated compatibility shims from the importable `src/arxiv_archive/` top level while preserving the exact old files for audit/history.

These files are archived, not deleted. Canonical modules now include `Formerly:` breadcrumbs pointing back to the old paths.

| Old path | Canonical path | Archive path | Status |
|---|---|---|---|
| `src/arxiv_archive/article_artifact_metrics.py` | `src/arxiv_archive/artifacts/metrics.py` | `archive/package-layout-shims/wave-01/src/arxiv_archive/article_artifact_metrics.py` | archived shim |
| `src/arxiv_archive/article_artifact_minimax.py` | `src/arxiv_archive/artifacts/minimax_boundary.py` | `archive/package-layout-shims/wave-01/src/arxiv_archive/article_artifact_minimax.py` | archived shim |
| `src/arxiv_archive/article_artifact_reducer.py` | `src/arxiv_archive/artifacts/reducer.py` | `archive/package-layout-shims/wave-01/src/arxiv_archive/article_artifact_reducer.py` | archived shim |
| `src/arxiv_archive/article_assets.py` | `src/arxiv_archive/artifacts/assets.py` | `archive/package-layout-shims/wave-01/src/arxiv_archive/article_assets.py` | archived shim |
| `src/arxiv_archive/article_evidence_bridge.py` | `src/arxiv_archive/artifacts/evidence_bridge.py` | `archive/package-layout-shims/wave-01/src/arxiv_archive/article_evidence_bridge.py` | archived shim |
| `src/arxiv_archive/llm_provider_config.py` | `src/arxiv_archive/llm/provider_config.py` | `archive/package-layout-shims/wave-01/src/arxiv_archive/llm_provider_config.py` | archived shim |

## Verification contract

- Direct imports of archived old paths must be absent from `src`, `tests`, and `scripts`.
- Canonical imports and focused behavior tests must pass.
- Archived files must remain inspectable in this directory.

## Non-goals

- No runtime behavior change.
- No live API calls.
- No graph writes or fact promotion.
- No deletion of historical source content.
