# M191 Parser Command Map

## Verdict

**M191 can expand parser readiness evidence using existing local M029 readiness artifacts, M031 catalog-backed replay artifacts, parser replay tests, and low-quality source tests.**

## Discovery evidence

- Parser command/input discovery: `gsd_exec[c8cf2b4d-18d1-4f2a-8c76-8e0feb498c72]`
- M029/M031 required input discovery: `gsd_exec[2c7c5d6c-d557-4792-b264-8b901629a554]`
- M029/M031 root listing: `gsd_exec[685a19b0-eec0-41c5-81d0-6c56f26d6c58]`

## Bounded parser expansion selection

Primary expansion surfaces:

1. M029 unified corpus readiness artifacts:
   - `data/article_corpora/m029-unified-corpus-v1/readiness-summary.json`
   - `data/article_corpora/m029-unified-corpus-v1/readiness-decision.json`
   - `data/article_corpora/m029-unified-corpus-v1/readiness-report.md`
   - `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json`

2. M031 catalog-backed replay artifacts:
   - `data/article_corpora/m031-catalog-backed-replay-v1/selection.json`
   - `data/article_corpora/m031-catalog-backed-replay-v1/source-acquisition-summary.json`
   - `data/article_corpora/m031-catalog-backed-replay-v1/loader-evidence-summary.json`
   - `data/article_corpora/m031-catalog-backed-replay-v1/source/`
   - `data/article_corpora/m031-catalog-backed-replay-v1/loader-evidence/`

3. Parser replay and low-quality source tests:
   - `tests/test_parser_replay_use_case.py`
   - `tests/test_parser_replay_adapters.py`
   - `tests/test_m031_catalog_backed_acquisition_loader.py`
   - `tests/test_m055deep_grobid_fulltext.py -k low_quality_source_criteria`

## Direct execution commands for S03

M029 readiness verify:

```bash
uv run python scripts/verify_m029_unified_readiness.py \
  --selection data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json \
  --readiness-summary data/article_corpora/m029-unified-corpus-v1/readiness-summary.json \
  --readiness-decision data/article_corpora/m029-unified-corpus-v1/readiness-decision.json \
  --readiness-report data/article_corpora/m029-unified-corpus-v1/readiness-report.md \
  --require-no-network \
  --require-no-import-flags \
  --check-dedupe-rule \
  --check-provenance \
  --write-verify-summary data/architecture-assessment/m191-m029-readiness-verify-summary.json
```

M031 catalog-backed replay verify:

```bash
uv run python scripts/verify_m031_catalog_backed_replay.py \
  --selection data/article_corpora/m031-catalog-backed-replay-v1/selection.json \
  --acquisition-summary data/article_corpora/m031-catalog-backed-replay-v1/source-acquisition-summary.json \
  --loader-summary data/article_corpora/m031-catalog-backed-replay-v1/loader-evidence-summary.json \
  --source-dir data/article_corpora/m031-catalog-backed-replay-v1/source \
  --loader-dir data/article_corpora/m031-catalog-backed-replay-v1/loader-evidence \
  --write-summary data/architecture-assessment/m191-m031-catalog-backed-replay-summary.json \
  --write-diagnostics data/architecture-assessment/m191-m031-catalog-backed-replay-diagnostics.jsonl \
  --write-report data/architecture-assessment/m191-m031-catalog-backed-replay-report.md
```

Parser and source-quality tests:

```bash
uv run pytest tests/test_parser_replay_use_case.py tests/test_parser_replay_adapters.py -q
uv run pytest tests/test_m031_catalog_backed_acquisition_loader.py -q
uv run pytest tests/test_m055deep_grobid_fulltext.py -q -k 'low_quality_source_criteria'
```

## Output mutation expectations

Allowed generated outputs:

- `data/architecture-assessment/m191-m029-readiness-verify-summary.json`
- `data/architecture-assessment/m191-m031-catalog-backed-replay-summary.json`
- `data/architecture-assessment/m191-m031-catalog-backed-replay-diagnostics.jsonl`
- `data/architecture-assessment/m191-m031-catalog-backed-replay-report.md`
- M191 summary artifacts under `data/architecture-assessment/`

Disallowed outputs:

- source-code edits;
- graph import artifacts;
- LadybugDB or production persistence writes;
- optimizer outputs;
- broad parser readiness claims outside M029/M031 bounded surfaces.

## Scope boundary

M191 can expand parser readiness evidence from M190's M027 local scope to bounded M029 readiness and M031 catalog-backed replay surfaces. It still cannot claim graph readiness or production readiness.
