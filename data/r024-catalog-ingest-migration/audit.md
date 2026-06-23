# M061 Ingest Migration Audit (M120 S01)

**Generated**: 2026-06-23 (M120 S01)  
**Source**: `scripts/m061_ingest_to_canonical_catalog.py` (688 lines)

## Overview

`scripts/m061_ingest_to_canonical_catalog.py` is a **post-M105 orphan script**:
- Lives in `scripts/` not `src/research_graph/`
- Imports only stdlib + feedparser (does NOT use any `research_graph.*` package primitives)
- Uses absolute paths via `ROOT = Path(__file__).resolve().parents[1]` (assumes script layout)
- References `scripts/verify_m025_article_catalog` directly via sys.path manipulation

**M105 wave-based refactor** renamed `arxiv_archive` → `research_graph`, but this script was never migrated. All other ingestion-related modules live under `src/research_graph/infrastructure/corpus/ingestion/` (loader.py, fetchers.py, logging.py).

## Function Inventory

| # | Function | Purpose | Target Module |
|---|----------|---------|---------------|
| 1 | `sha256_file(path)` | SHA256 digest of file | `catalog_ingest.py::sha256_file` |
| 2 | `normalize_arxiv_id(value)` | Strip `.pdf`, whitespace | `catalog_ingest.py::normalize_arxiv_id` |
| 3 | `normalize_category(value)` | Lowercase + dot→dash | `catalog_ingest.py::normalize_category` |
| 4 | `report_bucket(category)` | Map to known bucket | `catalog_ingest.py::report_bucket` |
| 5 | `catalog_pdf_count(root)` | Count PDFs in canonical catalog | `catalog_ingest.py::catalog_pdf_count` |
| 6 | `load_selected_ids(m061_root)` | Read `selected-2hop-papers.json` from anchors | `catalog_ingest.py::load_selected_ids` |
| 7 | `load_pdf_paths(m061_root)` | Find all anchor PDFs | `catalog_ingest.py::load_pdf_paths` |
| 8 | `invert_anchor_membership(anchor_ids)` | arxiv_id → [anchor_ids] | `catalog_ingest.py::invert_anchor_membership` |
| 9 | `existing_catalog_pdf(arxiv_root, arxiv_id)` | Find existing PDF for arxiv_id | `catalog_ingest.py::existing_catalog_pdf` |
| 10 | `parse_retry_after(value)` | Parse Retry-After header | `catalog_ingest.py::parse_retry_after` |
| 11 | `RequestPacer` (class) | arxiv API rate limiting | `catalog_ingest.py::RequestPacer` |
| 12 | `arxiv_query_url(arxiv_id)` | Build arxiv API URL | `catalog_ingest.py::arxiv_query_url` |
| 13 | `fetch_arxiv_metadata(...)` | HTTP fetch with retry/backoff | `catalog_ingest.py::fetch_arxiv_metadata` |
| 14 | `build_article_record(...)` | Build article.json dict | `catalog_ingest.py::build_article_record` |
| 15 | `write_article_record(...)` | Write article.json | `catalog_ingest.py::write_article_record` |
| 16 | `update_index_if_exists(...)` | Rebuild article_catalog/index.json | `catalog_ingest.py::update_index_if_exists` |
| 17 | `ingest_catalog(...)` | Main ingest orchestration | `catalog_ingest.py::ingest_catalog` |
| 18 | `per_anchor_counts(records)` | Group by anchor | `catalog_ingest.py::per_anchor_counts` |
| 19 | `render_report(...)` | Markdown report generation | `catalog_ingest.py::render_report` |
| 20 | `parse_args(argv)` | argparse | `catalog_ingest.cli::parse_args` (CLI subcommand) |
| 21 | `main(argv)` | Entry point | `catalog_ingest.cli::main` |

## Dataclass Inventory

| Dataclass | Fields | Target |
|-----------|--------|--------|
| `ArxivMetadata` | arxiv_id, category, title, source, fallback, error | `catalog_ingest::ArxivMetadata` |
| `ApiMetrics` | requests_made, rate_limit_429s, pacing_delay_seconds, retry_delay_seconds, failures | `catalog_ingest::ApiMetrics` |
| `IngestRecord` | arxiv_id, anchor_ids, source_pdf, dest_pdf, category, title, status, fallback, source_sha256, dest_sha256, message | `catalog_ingest::IngestRecord` |
| `IngestResult` | records, selected_total, discovered_pdf_total, unique_arxiv_ids, before_catalog_pdf_count, after_catalog_pdf_count, api_metrics, index_updated, index_entries, index_diagnostics | `catalog_ingest::IngestResult` |

## Safety Flags Mapping

| M061 Flag | Default | research_graph Equivalent |
|-----------|---------|-------------------------|
| `external_network_authorized` (SAFETY_OVERRIDE=True for M061) | False (SAFETY_DEFAULTS) | None — explicit user opt-in pattern |
| `graph_writes_authorized` | False | `CATALOG_SAFETY_FLAGS["graph_import_allowed"]` |
| `production_import_authorized` | False | `CATALOG_SAFETY_FLAGS["production_import_attempted"]` |
| `fact_promotion_authorized` | False | (new) |
| `llm_calls_authorized` | False | (new) |
| `metadata_manifests_embed_raw_text` | False (CATALOG_SAFETY_FLAGS) | CATALOG_SAFETY_FLAGS |
| `metadata_manifests_embed_raw_binary` | False | CATALOG_SAFETY_FLAGS |
| `graph_import_allowed` | False | CATALOG_SAFETY_FLAGS |
| `production_ladybugdb_write_allowed` | False | CATALOG_SAFETY_FLAGS |
| `trusted_kg_import_allowed` | False | CATALOG_SAFETY_FLAGS |
| `production_import_attempted` | False | CATALOG_SAFETY_FLAGS |
| `ladybugdb_written` | False | CATALOG_SAFETY_FLAGS |
| `raw_text_embedded_in_metadata` | False | CATALOG_SAFETY_FLAGS |
| `raw_binary_embedded_in_metadata` | False | CATALOG_SAFETY_FLAGS |
| `network_fetch_required_for_pipeline_phase` | False | CATALOG_SAFETY_FLAGS |

## CLI Surface

Current: `python scripts/m061_ingest_to_canonical_catalog.py [--no-index]`

Future options:
- `python -m research_graph.infrastructure.corpus.ingestion.cli ingest [--no-index]` (typer)
- `scripts/ingest_to_canonical_catalog.py [--no-index]` (thin wrapper, default entry)

**Decision**: thin wrapper `scripts/ingest_to_canonical_catalog.py` as default entry;
legacy `scripts/m061_ingest_to_canonical_catalog.py` becomes deprecation delegate.

## External Dependencies

| Dependency | Status | Migration Action |
|------------|--------|-------------------|
| `feedparser` | already in pyproject | Reuse as-is |
| `urllib` (stdlib) | N/A | Reuse as-is |
| `hashlib` (stdlib) | N/A | Reuse as-is |
| `research_graph.infrastructure.corpus.sources.arxiv_client.ArxivClient` | EXISTING in package | Reuse via package import |
| `scripts/verify_m025_article_catalog.rebuild_index_from_articles` | External script | Wrap with proper package import |

## Risk Areas

1. **`scripts/verify_m025_article_catalog` import**: Currently uses `sys.path.insert` hack.
   Migration: move `rebuild_index_from_articles` into research_graph package
   (`research_graph.infrastructure.corpus.index_rebuilder`) OR keep as script
   with explicit relative path.

2. **SAFETY_OVERRIDE pattern**: M061 uses dict with `external_network_authorized=True`.
   Migration: convert to typed `SafetyOverride` dataclass with explicit
   `UserAuthorization` field documenting scope + reason.

3. **PDFDownloader reuse**: M061 copies local PDFs (no download). If future M061
   needs to download, can use existing
   `research_graph.infrastructure.corpus.ingestion.fetchers.PDFDownloader`.

## Migration Surface Area

| Component | M061 Lines | Migration Target |
|-----------|-----------|-----------------|
| Constants (URLs, intervals) | ~15 | `catalog_ingest::constants` |
| Safety dicts | ~25 | `catalog_ingest::safety` module |
| Dataclasses | ~40 | `catalog_ingest::dataclasses` |
| Helper functions | ~120 | `catalog_ingest::helpers` |
| HTTP/Pacer | ~80 | `catalog_ingest::arxiv_client` (use package) |
| Build article | ~70 | `catalog_ingest::article_builder` |
| Ingest orchestration | ~150 | `catalog_ingest::ingest_catalog` |
| Report rendering | ~100 | `catalog_ingest::report` |
| CLI | ~40 | `catalog_ingest::cli` |

**Total**: ~688 lines split across ~9 modules within catalog_ingest package.

## What Stays in Script (thin delegate)

The legacy `scripts/m061_ingest_to_canonical_catalog.py` becomes:

```python
#!/usr/bin/env python3
"""DEPRECATED: Use scripts/ingest_to_canonical_catalog.py instead.

This script delegates to research_graph.infrastructure.corpus.ingestion.catalog_ingest.
Preserved for trajectory check + audit trail (see scripts/check_project_trajectory.py).
"""
import warnings
import sys
from pathlib import Path

warnings.warn(
    "scripts/m061_ingest_to_canonical_catalog.py is deprecated. "
    "Use scripts/ingest_to_canonical_catalog.py or "
    "research_graph.infrastructure.corpus.ingestion.catalog_ingest directly.",
    DeprecationWarning,
    stacklevel=2,
)
# Delegate to new entry point
sys.path.insert(0, str(Path(__file__).resolve().parent))
from ingest_to_canonical_catalog import main

if __name__ == "__main__":
    raise SystemExit(main())
```

## Next Steps (S02+)

- S02: Create `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py`
- S02: Add typed dataclasses (`SafetyOverride`, `IngestOptions`, `IngestResult`)
- S02: Use `PDFDownloader` + `ValidationLogger` from existing package
- S02: Document all 9 modules clearly
- S03: Wire into `__init__.py` + add tests
- S04: Add CLI entry point `scripts/ingest_to_canonical_catalog.py`
- S05: Convert legacy script to delegate
- S06: Migration report + R024 close-out