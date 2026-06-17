# Current Package Map Before `research_graph` Rename

## Current distribution and package identity

`pyproject.toml` currently defines the distribution as:

```toml
[project]
name = "arxiv-daily-archive"

[tool.setuptools.packages.find]
where = ["src"]
```

The importable Python package is currently:

```text
src/arxiv_archive/
```

This means imports use:

```python
from arxiv_archive.artifacts.metrics import ...
from arxiv_archive.llm.provider_config import ...
```

## Problem

`arxiv_archive` names an early data-source-oriented idea, not the current domain. The project now covers:

- scientific corpus ingestion;
- paper parsing and source normalization;
- paper evidence bundles;
- article artifacts and asset metadata;
- page indexes and retrieval tables;
- graph-readiness review and export;
- staging/import boundaries;
- LLM helper boundaries and provider configuration;
- evaluation and quality gates;
- workflow orchestration and CLI.

So `arxiv_archive` is too narrow and misleading.

## Current subpackages

```text
src/arxiv_archive/
├── artifacts/
│   ├── assets.py
│   ├── evidence_bridge.py
│   ├── metrics.py
│   ├── minimax_boundary.py
│   └── reducer.py
├── assets/
│   ├── provenance.py
│   └── registry.py
├── chunking/
│   ├── chunker.py
│   ├── figure_units.py
│   └── table_units.py
├── identity/
│   ├── canonicalization.py
│   └── dedup.py
├── indexing/
│   ├── navigation.py
│   └── page_index.py
├── ingestion/
│   ├── fetchers.py
│   ├── loader.py
│   └── logging.py
├── llm/
│   └── provider_config.py
├── parsing/
│   ├── normalization.py
│   ├── parser.py
│   └── structure.py
├── quality/
│   ├── baselines.py
│   ├── maintainability_report.py
│   ├── riskratchet_adapter.py
│   ├── scopes.py
│   └── thresholds.py
└── staging/
    ├── graph_candidates.py
    └── import_boundary.py
```

## Remaining top-level pressure

There are still about 63 top-level `.py` files under `src/arxiv_archive/`. The article-related ones still visible at top level are:

```text
article_artifact_worker.py
article_artifacts.py
article_batch_validation.py
article_links_dedup.py
article_loader.py
article_page_index.py
article_retrieval_tables.py
```

Only some of these are implementations; `article_loader.py` is already a shim to ingestion but has not yet been archived.

## Already archived in M086 wave 01

These old top-level shims were moved out of importable `src/arxiv_archive/` and preserved under `archive/package-layout-shims/wave-01/`:

```text
article_artifact_metrics.py
article_artifact_minimax.py
article_artifact_reducer.py
article_assets.py
article_evidence_bridge.py
llm_provider_config.py
```

Canonical replacements currently still live under `arxiv_archive`:

```text
src/arxiv_archive/artifacts/metrics.py
src/arxiv_archive/artifacts/minimax_boundary.py
src/arxiv_archive/artifacts/reducer.py
src/arxiv_archive/artifacts/assets.py
src/arxiv_archive/artifacts/evidence_bridge.py
src/arxiv_archive/llm/provider_config.py
```

## Current archive rule

After M086, old top-level files should not remain as permanent shims. Once canonical code is verified, old files leave `src/` and are preserved under `archive/package-layout-shims/wave-XX/`. Canonical modules include a breadcrumb such as:

```text
Formerly: src/arxiv_archive/article_assets.py
```

## Implication for rename

Renaming `arxiv_archive` to `research_graph` should not be done by a single global search/replace. The current package already contains multiple bounded contexts, so the rename should happen by context wave with targeted verification.
