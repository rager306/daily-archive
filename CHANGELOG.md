# Changelog

## Unreleased

### Rust v2 (2026-07-26)

- **Architecture guardrail CI rewritten** for Rust hexagonal layout (D131).
  Old Python M044/M045 workflow was broken after Python→legacy move
  (`scripts/` and root `pyproject.toml` gone). New workflow: cargo fmt/check/clippy
  + dependency-direction check. Local mirror: `scripts/verify_rust_architecture.sh`.
- **Pre-commit restored**: `.pre-commit-config.yaml` with cargo-fmt + cargo-check
  (was missing; every commit required `PRE_COMMIT_ALLOW_NO_CONFIG=1`).
- **cargo fmt** applied across all crates; unused imports cleaned.
- `da load-snapshot` + `da graph-stats` CLI commands (snapshot round-trip).
- `da batch-ingest` with `.sgsnap` export (Solution B durability).
- GROBID title extraction bug fixed (TEI attributes).
- `da-graph` filled: Cypher query builders + schema DDL (8 unit tests).
- Integration tests for batch_ingest with mock ports (5 tests, no live services).
- ADR-INDEX.md + ADR-037 status marked partially superseded by 040/041.
- README rewritten for Rust v2 (was fully Python-era).
- **GROBID TEI sections + citations extraction**: `extract_sections` (85 from
  real paper), `extract_citations` (80 citations with DOI/arXiv id). Closed
  TODO debt (sections/citations were discarded). 7 new tests.
- **`da query` CLI command**: count, by-arxiv, by-vid, orphans, without-evidence,
  citation-hops. Wires da-graph (was dead library).
- **Sections + citations persisted to graph**: IngestUseCase now writes
  section_count, citation_count as node properties and creates Citation nodes
  + CITES edges for resolvable citations. IngestResult reports counts.
- **Bibliographic edge types in domain**: `bibliographic::CITES` / `CITED_BY` /
  `CO_AUTHORED` constants in da-domain. Separated from ADR-038's 18 extracted
  RelationTypes. da-graph queries now reference the domain constant.
- Clippy clean across all crates; CI clippy uses `--no-deps` (vendor Samyama
  has pre-existing warnings we don't own).
- Pre-commit cargo-fmt now auto-formats (was failing on `--check`).
- **Idempotent Citation nodes**: `find_node_by_string_property` added to
  DirectGraphStore trait. Ingest now checks for existing Citation before
  creating, preventing duplicate nodes when multiple papers cite the same
  reference. Citation nodes now get valid_from + schema_version.
- **Phase 3 extraction started**: Extractor port (da-ports), RuleBasedExtractor
  adapter (da-adapters, section heuristics + keyword patterns), ExtractionUseCase
  (da-application, parse → extract → write Entity nodes). `da extract --id` CLI.
  Idempotent Entity node creation. 9 new tests. End-to-end: 2602.11757 → 2 entities.

### Hygiene (earlier)

- Remove obsolete root briefs and empty `Plans/`.
- Document local garbage policy (`doc/REPO-HYGIENE.md`).

### ETL / Wave B (frozen under legacy/)

- M271–M284: quality n-contract, hybrid bodies, import-hold inventory, evidence chain.
- Import remains locked (D127); deploy extract path remains `header_priority`.
- Full Python stack frozen under `legacy/` — not on the Rust runtime path.

## Earlier history

Milestone-level detail lives in git history and GSD phase summaries. Prefer:

- `doc/adr/ADR-INDEX.md` for binding architecture decisions
- `doc/PERSISTENCE-ANALYSIS.md` for graph durability model
- `artifacts/etl/` for historical Python ETL evidence
