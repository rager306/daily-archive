# Requirements

## Active

| ID | Description | Class | Milestone | Status | Notes |
|----|-------------|-------|-----------|--------|-------|
| R001 | CLI help info | core-capability | M001 | validated | Typer app exposes project purpose, artifact paths, exit codes, and non-goals |
| R002 | CLI `--date` analysis | core-capability | M001 | validated | Wires `--date` to ArxivClient producing `DailyAnalysis` result |
| R003 | JSON result in sessions | primary-user-loop | M001 | validated | `write_session_json()` writes `~/research/ops/sessions/YYYY-MM-DD.json` |
| R004 | Save full list of papers | core-capability | M001 | validated | `write_daily_artifacts()` saves full `papers.json` and `scored.json` |
| R005 | Per-paper artifacts | primary-user-loop | M001 | validated | `write_paper_artifacts()` creates `paper.json` and `scored.json` per arxiv-id |
| R006 | Topic overview aggregates | primary-user-loop | M001 | validated | `build_overview_payload()` aggregates categories, keywords, top papers |
| R007 | Transparent score breakdown | quality-attribute | M001 | validated | `score_breakdown` statistics in per-paper and daily overview |
| R008 | Queue state file | operability | M001 | validated | `write_queue_state()` tracks running → done/empty/failed lifecycle |
| R009 | Idempotent reruns | operability | M001 | validated | Last-writer-wins overwrite across all artifact writers |
| R010 | Rust-portable contracts | integration | M001 | validated | Explicit serializer functions, portable exit-code vocabulary |
| R011 | Follow style guide/lint | quality-attribute | M001 | validated | Zero Ruff lint findings, full pytest suite passes |
| R012 | Empty day handling | core-capability | M001 | validated | `status="empty"` with exit 0, valid empty JSON arrays |
| R013 | Pytest contract coverage | quality-attribute | M001 | validated | Offline subprocess tests for help, JSON, empty, failure, rerun |
