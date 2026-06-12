---
id: T02
parent: S01
milestone: M059-y6osma
key_files:
  - scripts/m059_build_manifest.py
  - scripts/m059_jsonschema_validate.py
  - artifacts/m054-pdf-acquisition/manifest.json
  - artifacts/m055-parser-benchmark/manifest.json
  - artifacts/m055deep-parser-benchmark/manifest.json
  - artifacts/m056-bfs-graph/manifest.json
  - artifacts/m057-fd-marker/manifest.json
  - artifacts/m058-plotextractor/manifest.json
key_decisions:
  - Use `source_uri` from acquisition logs when available and canonical arXiv PDF URIs otherwise.
  - Keep all generated manifest safety defaults explicit false.
  - Use per-PDF output templates for parser outputs and batch output paths for batch-level diagnostics.
duration: 
verification_result: passed
completed_at: 2026-06-12T10:17:37.258Z
blocker_discovered: false
---

# T02: Built the retroactive manifest generator, generated manifests for M054-M058, and added a generic jsonschema validator.

**Built the retroactive manifest generator, generated manifests for M054-M058, and added a generic jsonschema validator.**

## What Happened

Implemented `scripts/m059_build_manifest.py` to generate idempotent retroactive manifests from the existing M054 acquisition log, M055 corpus manifests, M056 cumulative corpus, M057 fd summaries, and M058 PlotExtractor summary. The generated manifests compute size and SHA-256 from local PDF bytes, declare parser expectations, and carry the five explicit false safety defaults. Implemented `scripts/m059_jsonschema_validate.py` to read a manifest, select a parser, resolve output paths, validate parser outputs against the declared schema, and emit per-PDF plus aggregate pass/fail statistics.

## Verification

Ran the generator and verified manifest counts: M054 5, M055 5, M055deep 20, M056 166, M057 166, M058 5. Ran the POC validator for M054 GROBID: aggregate total=5 passed=5 failed=0 missing=0. Targeted pytest also passed 8 tests.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/m059_build_manifest.py --batch all` | 0 | ✅ pass | 2700ms |
| 2 | `uv run python scripts/m059_jsonschema_validate.py --manifest=artifacts/m054-pdf-acquisition/manifest.json --parser=grobid` | 0 | ✅ pass | 2800ms |
| 3 | `uv run pytest tests/test_m059_s01.py -q` | 0 | ✅ pass | 9100ms |

## Deviations

Generated six manifest files because the plan lists M055 and M055deep as separate outputs even though it calls the range five retroactive manifests.

## Known Issues

None.

## Files Created/Modified

- `scripts/m059_build_manifest.py`
- `scripts/m059_jsonschema_validate.py`
- `artifacts/m054-pdf-acquisition/manifest.json`
- `artifacts/m055-parser-benchmark/manifest.json`
- `artifacts/m055deep-parser-benchmark/manifest.json`
- `artifacts/m056-bfs-graph/manifest.json`
- `artifacts/m057-fd-marker/manifest.json`
- `artifacts/m058-plotextractor/manifest.json`
