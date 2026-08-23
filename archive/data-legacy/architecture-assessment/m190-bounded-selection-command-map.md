# M190 Bounded Selection Command Map

## Verdict

**M190 bounded execution selection is the existing local M027 six-article source/conversion corpus, with M030 validate-only and representative metric/ablation tests as companion gates.**

## Discovery evidence

- Selection and command discovery: `gsd_exec[9a82fa2b-5945-4f53-917a-fab75c732199]`
- M029 runtime input discovery: `gsd_exec[4bf8f390-ba4e-46cf-9adf-48a44002f225]`

## Bounded selection

Primary bounded corpus:

- `data/article_corpora/m027-mixed-source-corpus-v1/selection.json`
- `data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-summary.json`
- `data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-report.md`

This scope is selected because it already has local source boundary evidence and supports a local-only current pipeline replay command with `--no-network` semantics.

Companion intake scope:

- `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json`
- `data/article_corpora/m029-pipeline-architecture-audit-v1/intake-report.md`
- `data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json`

This scope is used only for M030 validate-only intake consistency, not for direct M029 runtime smoke execution.

## Direct execution command surfaces

M027 bounded replay:

```bash
uv run python scripts/replay_m027_current_pipeline_baseline.py \
  --no-network \
  --output-dir data/architecture-assessment/m190-m027-current-pipeline-replay
```

M027 source boundary verifier:

```bash
uv run python scripts/verify_m027_source_acquisition_boundary.py
```

M030 validate-only:

```bash
uv run python scripts/verify_m030_requested_ref_intake.py \
  --selection data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json \
  --report data/article_corpora/m029-pipeline-architecture-audit-v1/intake-report.md \
  --catalog-index data/article_catalog/index.json \
  --m028-selection data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json \
  --validate-only
```

Representative metric/ablation/boundary tests:

```bash
uv run pytest tests/test_extraction_benchmark.py tests/test_evaluation_benchmark.py tests/test_dspy_extraction_boundary.py -q
uv run pytest tests/test_m055deep_grobid_fulltext.py -q -k 'low_quality_source_criteria'
```

## Candidate excluded from direct M190 execution

`run_m029_unified_loader_runtime_smoke.py` is present, but direct execution requires `--conversion-summary` and `--source-summary`. No M029 summary JSON inputs were found under `data/article_corpora/m029-pipeline-architecture-audit-v1` during discovery, so M190 will not run it directly. Existing M029 tests remain representative coverage.

## Output mutation expectations

Allowed generated outputs:

- New M190 artifacts under `data/architecture-assessment/`.
- New M027 replay output directory under `data/architecture-assessment/m190-m027-current-pipeline-replay/`.
- Possible regenerated M027 source-acquisition summary/report if the verifier owns those files.

Disallowed outputs:

- Source code edits.
- Graph import files.
- LadybugDB/production persistence writes.
- DSPy/RLM optimizer output.
