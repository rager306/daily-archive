# M188 Validator Command Map

## Verdict

**S02 and S03 can proceed with existing local validators and focused tests.**

## Discovery evidence

- Command surface discovery: `gsd_exec[be8c2516-a371-46bd-98e0-f54138252091]`
- Real corpus artifact path discovery: `gsd_exec[9405a971-1a37-49b0-8643-cb794ccf47c5]`

## Present files

All candidate files from the S01 scope lock are present:

- `scripts/verify_article_catalog.py`
- `scripts/verify_m030_requested_ref_intake.py`
- `scripts/run_m029_unified_replay.py`
- `scripts/verify_m027_source_acquisition_boundary.py`
- `tests/test_m029_post_validation_remediation.py`
- `tests/test_m029_loader_runtime_smoke.py`
- `tests/test_m031_chunk_evidence_replay.py`
- `tests/test_m036_real_corpus_no_write_smoke.py`
- `tests/test_m036_real_corpus_smoke_audit.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`

## S02 validate-only and low mutation risk commands

Run these first:

```bash
uv run python scripts/verify_article_catalog.py
uv run python scripts/verify_m030_requested_ref_intake.py \
  --selection data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json \
  --report data/article_corpora/m029-pipeline-architecture-audit-v1/intake-report.md \
  --catalog-index data/article_catalog/index.json \
  --m028-selection data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json \
  --validate-only
uv run pytest tests/test_m029_post_validation_remediation.py -q
uv run pytest tests/test_m029_loader_runtime_smoke.py -q
uv run pytest tests/test_m036_real_corpus_no_write_smoke.py tests/test_m036_real_corpus_smoke_audit.py -q
```

## S03 readiness probe commands

Run these after S02 baseline is known:

```bash
uv run pytest tests/test_m031_chunk_evidence_replay.py -q
uv run python scripts/verify_m027_source_acquisition_boundary.py
```

`verify_m027_source_acquisition_boundary.py` is local-only by help text, but may refresh configured report artifacts when run with defaults. Treat any output update as validation evidence, not as source behavior change.

## Commands requiring explicit output planning

`run_m029_unified_replay.py` requires:

- `--selection`
- `--runtime-smoke-summary`
- `--evidence-dir`
- `--output-dir`

It writes compact metadata-only replay artifacts. Do not run it in S02 unless an explicit temp output directory is chosen. Prefer tests first.

## Artifact roots discovered

- M029 root exists: `data/article_corpora/m029-pipeline-architecture-audit-v1`
- M029 inputs found: `selection.json`, `intake-report.md`
- M028 root exists: `data/article_corpora/m028-universal-loader-runtime-smoke-v1`
- M028 inputs found: `selection.json` and smoke replay closeout artifacts
- M027 root exists: `data/article_corpora/m027-mixed-source-corpus-v1`

## Readiness labels for follow-up waves

Use these labels consistently:

- `catalog_ready`
- `intake_ready`
- `source_boundary_ready`
- `parser_ready`
- `chunk_ready`
- `low_quality_source`
- `graph_not_ready`
- `not_evaluated`

Graph readiness remains `graph_not_ready` unless a later milestone proves graph import and persistence readiness.
