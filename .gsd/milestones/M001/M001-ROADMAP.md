# M001: Cron-safe arXiv article analysis for Hermes

**Vision:** Сделать первый стабильный слой проекта: cron-safe CLI-анализатор arXiv за день, который Hermes-agent может запускать по расписанию, читать JSON-итоги и использовать накопленные данные для последующей тематической калибровки.

## Success Criteria

- Agent can learn project usage from CLI help without reading source files.
- Hermes can consume a stable JSON result for an explicit date.
- The full analyzed daily paper set and per-paper reusable artifacts are stored locally.
- Topic overview and scoring breakdown support interest calibration.
- Cron-safe empty, failed and rerun behaviors are verified.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: Agent can run top-level help and command help and see project purpose, Hermes usage, options, artifacts, exit codes, examples and out-of-scope items.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: CLI can analyze a specified date through existing arXiv, keyword and scoring code and produce a normalized daily analysis object including empty-day handling.

- [x] **S03: S03** `risk:high` `depends:[]`
  > After this: After a run, Hermes can read `~/research/ops/sessions/YYYY-MM-DD.json`; daily analysis files include full papers, scored papers and overview skeleton.

- [x] **S04: S04** `risk:medium` `depends:[]`
  > After this: For each analyzed article, local reusable files are created; overview shows category counts, top keywords, top papers and score breakdowns for interest calibration.

- [x] **S05: S05** `risk:high` `depends:[]`
  > After this: A verification run proves help, successful JSON output, empty day, failed state and same-date rerun behavior; pytest and ruff pass.

## Boundary Map

### S01 → S02

Produces:
- Typer CLI grammar and help contract for `arxiv_archive` and `run`.
- Documented option semantics for `--date` and `--json`.
- Initial portable exit-code vocabulary.

Consumes:
- Existing `src/arxiv_archive/__main__.py` entrypoint.

### S02 → S03

Produces:
- Normalized in-memory daily paper analysis shape from explicit date input.
- Stable use of existing arXiv, keyword and scoring modules.
- Clear distinction between `done` and `empty` daily analysis outcomes.

Consumes:
- S01 CLI command shape.

### S03 → S04

Produces:
- JSON session result schema under `~/research/ops/sessions/YYYY-MM-DD.json`.
- Daily artifacts under `~/research/analysis/YYYY-MM-DD/` including full paper list.
- Portable JSON serialization conventions.

Consumes:
- S02 normalized analysis output.

### S04 → S05

Produces:
- Per-paper artifact layout under `~/research/papers/{arxiv-id}/`.
- Overview schema for categories, keywords, top papers and score breakdown.
- Reusable stored analysis data for later preference calibration.

Consumes:
- S03 daily JSON/artifact writer.

### S05 → downstream milestones

Produces:
- Verified cron-safe behavior: help, run result, empty, failed and rerun contracts.
- Evidence that external CLI/JSON/file contracts are stable enough for Hermes and future Rust rewrite.

Consumes:
- S01-S04 public contracts and artifacts.
