# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.2.0] — 2026-05-16

### Changed

- **md_converter**: Replaced pymupdf with arxiv2md REST API + Marker CLI fallback
  - Primary: arxiv2md.org (fast, <1 sec, parses HTML)
  - Fallback: Marker CLI for pre-2020 papers (no HTML available)
- PDF downloader and md_converter separated into distinct modules

### Fixed

- Subprocess leak on Marker timeout (process now properly killed)
- Nested event loop bug in `convert_sync()`

## [0.1.0] — 2026-05-15

### Added

- Full 6-R pipeline: Record → Reduce → Score → Summarize → Deliver
- arXiv client (Record stage)
- Semantic Scholar enricher (Reduce stage)
- YAKE keyword extractor (Reduce stage)
- Scoring engine with weighted factors: citations, recency, novelty, preference
- MiniMax summarizer (Summarize stage) — HEADLINE/WHAT_IT_DOES/WHY_IT_MATTERS/ANALOGY
- PDF downloader
- Telegram delivery
- CLI entry point with session capture
- Integration tests
- Property-based tests with Hypothesis + Adaptix data loading

### Notes

- Phase 1 (Python prototype) complete
- Phase 2: Graphify integration, community detection, bridge detection
- Phase 3: Preference learning from user feedback
- Phase 4: Weekly cleanup, MOC updates
- Future: Go production rewrite (CPU-efficient)
