---
phase: M001
phase_name: Cron-safe arXiv article analysis for Hermes
project: daily-archive
generated: 2026-05-17T10:00:00.000Z
counts:
  decisions: 6
  lessons: 4
  patterns: 4
  surprises: 2
missing_artifacts: []
---

# M001 Learnings

## Decisions

### Typer CLI over argparse
Chose Typer for the CLI boundary because it produces self-documenting help with type-annotated option semantics, enabling Hermes/cron agents to discover the contract without reading source. argparse would have required manual help text maintenance.

### Pure function boundary (run_analysis) without persistence
S02 implemented `run_analysis()` as a pure function returning `DailyAnalysis` with no I/O side effects. This separates the analysis logic from serialization, enabling S03 to add JSON output without modifying the core analysis path. An alternative (persisting inside `run_analysis`) would have created a tangled dependency.

### Explicit serializer functions over dataclasses.asdict()
S03 used explicit serializer functions rather than `dataclasses.asdict()` to ensure Rust-portable JSON (no Python datetime objects, no dataclass metadata, snake_case keys). This was essential for the Hermes contract that requires language-agnostic JSON.

### Compatibility wrapper pattern (run_pipeline)
Preserved `run_pipeline()` as a compatibility wrapper around `run_analysis()` + `save_session()` instead of removing it. This allowed S02 to add the pure boundary without breaking existing callers or the legacy markdown session path.

### Idempotent same-date file overwrite
All artifact writers (session, daily, per-paper, queue state) use last-writer-wins idempotent overwrite semantics. This is the simplest safe strategy for same-date cron reruns without requiring manual cleanup.

### Offline subprocess contract testing
S05 used subprocess tests against `uv run python -m arxiv_archive` to cover the exact entrypoint Hermes/cron agents use, without live network dependencies. This provides genuine end-to-end verification of the public contract.

## Lessons

### setuptools src-layout packaging is required for public entrypoints
The `uv run python -m arxiv_archive` entrypoint only works if pyproject.toml includes setuptools build metadata (`build_backend` + `packages.find.where`). Relying only on pytest's `pythonpath` configuration leaves the public module entrypoint unimportable.
Source: M001-ROADMAP.md/Architecture Decisions

### Typer version constraints on datetime.date parameter types
The installed Typer version rejects `datetime.date` as a parameter type at command construction time. String-based `--date` parsing with manual validation is the practical workaround. This is a version-dependent constraint that could affect future upgrades.

### GitNexus worktree isolation degrades in gitfile-based worktrees
GitNexus impact/detect-changes checks are degraded when the worktree uses gitfile-based isolation (`.git` is a file pointing to the parent repo). Running with explicit GIT_DIR/GIT_WORK_TREE can partially work around this.
Source: S01-SUMMARY.md/Known Limitations

### Ruff lint exceptions for pre-existing test file issues
The `tests/test_analysis.py` file has pre-existing ruff forward-reference annotation issues. Rather than weakening lint settings, these should be fixed in a separate cleanup task. A CI exception (`# noqa`) was documented as a known limitation rather than a permanent solution.
Source: S03-SUMMARY.md/Follow-ups

## Patterns

### Normalized in-memory boundary with typed status literal
A frozen dataclass (`DailyAnalysis`) with `status: Literal["done", "empty"]` provides a clean, typed, testable boundary between the CLI layer and the persistence layer. The typed literal enables exhaustiveness checking in downstream consumers.
Source: S02-SUMMARY.md/Patterns Established

### Compatibility aliases in serialized payloads
Including both `id` (Python internal) and `paper_id` (explicit public key) in serialized payloads bridges existing tests and new language-agnostic consumers. Similarly, both singular and plural count keys (`papers_count`/`paper_count`) maintain backward compatibility while signaling the new public schema.
Source: S03-SUMMARY.md/Patterns Established

### Queue state lifecycle (running → done/empty/failed)
A queue state file tracking explicit transitions (`running` → `done`/`empty`/`failed`) enables Hermes to detect stale runs, skip already-processed dates, and handle failures gracefully. This is the standard pattern for cron-safe stateful pipelines.
Source: S05-SUMMARY.md/Key Changes

### TDD red-green on CLI contract
Writing the subprocess test first (proving the expected help output, exit code, or JSON shape), then implementing to pass, ensures the contract is never accidentally broken. This is especially important for public CLI contracts that external agents depend on.
Source: S01-SUMMARY.md/Patterns Established

## Surprises

### CLI --date is a top-level argument, not a subcommand
The plan showed `--date` as a subcommand argument, but Typer's design naturally placed it as a top-level argument on the `run` command. This was a cosmetic deviation — the contract is fully satisfied — but required updating the documentation expectations.
Source: M001-VALIDATION.md/Cross-Slice Integration (S01→S02 note)

### uv.lock required refresh after pyproject.toml packaging metadata changes
Adding setuptools src-layout packaging metadata to pyproject.toml triggered a `uv.lock` refresh that was not anticipated. This is a normal consequence of packaging changes but added an unexpected file to the diff.
Source: S01-SUMMARY.md/Deviations
