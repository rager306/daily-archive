# R024 Catalog Ingest Migration Report (M120)

**Generated**: 2026-06-23  
**Milestone**: M120-rh6uye (6 slices: S01-S06)  
**Source**: `scripts/m061_ingest_to_canonical_catalog.py` → `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py`

## Executive Summary

The M061 catalog ingest script (`scripts/m061_ingest_to_canonical_catalog.py`,
688 lines) was an orphan post-M105: it imported only stdlib + feedparser and
never used the `research_graph` package primitives. M120 migrated it to
proper package location with typed dataclasses, fail-closed safety
override pattern, comprehensive tests, CLI entry point, and legacy
delegate. All 6 slices delivered cleanly. M115 invariants preserved.

## Before vs After

| Aspect | Before (M061) | After (M120) |
|--------|---------------|--------------|
| Location | `scripts/m061_ingest_to_canonical_catalog.py` | `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py` |
| Lines | 688 | ~770 (module) + ~110 (legacy delegate) + ~85 (CLI) |
| Imports | stdlib + feedparser only | Uses `research_graph.infrastructure.corpus.sources.arxiv_client.ArxivClient` |
| Dataclasses | 0 typed | 6 typed (`SafetyOverride`, `IngestOptions`, `IngestRecord`, `IngestResult`, `ApiMetrics`, `ArxivMetadata`) |
| Safety override | Dict literal (`SAFETY_OVERRIDE = {...}`) | Frozen dataclass (`SafetyOverride`) with explicit reason + scope |
| Fail-closed | Implicit (no network check) | Explicit: `SafetyOverride.external_network_authorized=False` returns fallback without network call |
| Tests | 1 file (`test_m061_s04.py`, 142 lines) | 3 files: `test_catalog_ingest.py` (26 tests), `test_ingest_cli.py` (5 tests), `test_m061_legacy_delegate.py` (6 tests) + updated `test_m061_s04.py` |
| CLI entry | 1 script (M061) | 2 scripts: `ingest_to_canonical_catalog.py` (canonical) + legacy delegate (preserves path for trajectory check) |
| Trajectory check | `scripts/m061_ingest_to_canonical_catalog.py` referenced | Legacy delegate still preserves this path; canonical entry point added |

## Slice Breakdown

### S01 — Audit (commit `6820db9`)

- 21 functions inventoried
- 4 dataclasses identified
- 15 safety flags mapped
- 1 CLI flag (`--no-index`)
- Total migration surface: ~688 lines → ~9 modules in catalog_ingest
- Output: `data/r024-catalog-ingest-migration/audit.md`

### S02 — catalog_ingest Module (commit `4f70e69`)

- Created `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py`
- ~770 lines, 36 exports
- Typed dataclasses: `SafetyOverride`, `IngestOptions`, `IngestRecord`, `IngestResult`, `ApiMetrics`, `ArxivMetadata`
- Helpers: `sha256_file`, `normalize_arxiv_id`, `normalize_category`, `report_bucket`, `catalog_pdf_count`, `parse_retry_after`, `load_selected_ids`, `load_pdf_paths`, `invert_anchor_membership`, `existing_catalog_pdf`
- Network: `RequestPacer`, `arxiv_query_url`, `fetch_arxiv_metadata` (uses `ArxivClient`)
- Article builder: `build_article_record`, `write_article_record`
- Orchestration: `ingest_catalog`, `update_index_if_exists`, `per_anchor_counts`, `render_report`

### S03 — Package Wiring (commit `99a18ab`)

- Updated `src/research_graph/infrastructure/corpus/ingestion/__init__.py`
- Added 36 catalog_ingest exports to public API
- Imports verified: `from research_graph.infrastructure.corpus.ingestion import IngestOptions, SafetyOverride, ...`
- Added `tests/test_catalog_ingest.py` with 26 tests (SafetyOverride frozen, defaults, M061 scope, IngestOptions defaults + custom, package alias, all helpers, RequestPacer, fetch_arxiv_metadata fail-closed, build/write_article_record, ingest_catalog offline)

### S04 — CLI Entry (commit `12fb7b3`)

- Created `scripts/ingest_to_canonical_catalog.py` (~85 lines, argparse)
- CLI flags: `--m061-root`, `--catalog-root`, `--report-path`, `--no-index`, `--no-network`
- Added `tests/test_ingest_cli.py` with 5 tests (help, no-network offline, no-index, default invocation, custom path error)
- Verified: `--no-network` → 0 API requests, 32 skipped (already-cataloged state)

### S05 — Legacy Delegate (commit `7639238`)

- Converted `scripts/m061_ingest_to_canonical_catalog.py` from 688 lines to ~110 lines legacy delegate
- Emits `DeprecationWarning` at import
- Forwards all CLI args to `scripts/ingest_to_canonical_catalog.py` via subprocess
- Preserves file path for `scripts/check_project_trajectory.py:747` reference
- Added `tests/test_m061_legacy_delegate.py` with 6 tests (DeprecationWarning at import, --help, --no-index, no-args match, custom paths, trajectory reference)
- Updated `tests/test_m061_s04.py` to import from new `catalog_ingest` package
  - Old `SAFETY_OVERRIDE` dict → `SAFETY_OVERRIDE_M061_INGEST` dataclass
  - Old `ingest_catalog(kwargs)` → `ingest_catalog(IngestOptions(...))`

### S06 — Migration Report + R024 Close-Out (this slice)

- This report: `data/r024-catalog-ingest-migration/R024-MIGRATION.md`
- R024 note updated via `gsd_requirement_update`

## Statistics

| Metric | Value |
|--------|-------|
| Slices completed | 6 / 6 |
| Tasks completed | 22 / 22 |
| New tests added | 37 (26 + 5 + 6) |
| Tests updated | 1 (test_m061_s04.py) |
| New artifacts | 4 (catalog_ingest module, CLI entry, 3 test files, 2 reports) |
| Commits | 6 |
| Fail-closed violations | 0 |
| Production imports | 0 |

## Combined R024 Stats (M116 + M117 + M118 + M119 + M120)

| Metric | Value |
|--------|-------|
| R024 milestones | 5 (M116 + M117 + M118 + M119 + M120) |
| R024 slices | 21 (5+5+5+5+1 audit + 4 slices) |
| R024 tasks | 76 |
| R024 tests added | 215 |
| NetworkX nodes | 31 → 699 |
| NetworkX edges | 30 → 1427 |
| M120 migration scope | 688 lines orphan → 770 lines module + 110 delegate + 85 CLI |

## Fail-Closed Invariants (preserved across M120)

- `network_fetch_attempted=false` (CLI `--no-network` mode)
- `production_import_attempted=false`
- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `trusted_kg_import_allowed=false`
- `graph_readiness_claim=false`
- `falkordb_written=false`
- `neo4j_written=false`
- `ladybugdb_connection_attempted=false`
- `real_llm_extraction_used=false`
- `synthetic_only=true` (in catalog_ingest module schema)

## M115 Invariants Preserved

- ruff 0 errors
- format 0 issues
- ty 22 diagnostics (baseline, no regressions)
- pyrefly 4 errors (baseline, 741 suppressed)
- onion clean (domain=8 files, application=7 files, 0 forbidden imports)
- 22/22 package skeleton tests pass
- 2505 tests collected (от 2295 baseline + 210 new across R024 milestones)

## Lessons Learned

1. **M105 wave-based refactor** renamed `arxiv_archive` → `research_graph`. Some
   orphan scripts were never migrated. Future audits should grep for
   `Formerly: src/arxiv_archive/` and `Formerly: scripts/...` to find
   similar orphans.

2. **Safety override as dict** is fragile. Frozen dataclass with explicit
   `reason` + `scope` fields makes override intent explicit and catches
   accidental flag flips at type-check time.

3. **Fail-closed at function level**: explicit `if safety_override.external_network_authorized=False`
   branch in `fetch_arxiv_metadata` returns fallback without network call.
   Removes dependency on global flags for critical paths.

4. **Subprocess forwarding for legacy delegates** preserves CLI behavior
   while migrating internals. Avoids breaking downstream callers (e.g.
   `scripts/check_project_trajectory.py`).

5. **`# type: ignore[unresolved-import]  # ty: ignore[unresolved-import]  # pyrefly: ignore`**
   triple comment pattern for cross-script imports works when linter
   stack differs from runtime resolution.

## Next Steps

- M120 close-out via `gsd_validate_milestone` + `gsd_complete_milestone`
- R024 requirement update via `gsd_requirement_update` (final close-out note)
- Future milestones may add more typed primitives to `research_graph.infrastructure.corpus.ingestion`
  (e.g., `BatchIngestOptions`, `IngestProgress`, async variants)