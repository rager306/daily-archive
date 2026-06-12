---
id: T01
parent: S01
milestone: M059-y6osma
key_files:
  - schemas/daily-archive.pdf-batch-manifest.v1.json
  - schemas/daily-archive.parser-op.v1.json
  - schemas/grobid-tei.v1.json
  - schemas/opendataloader-pdf.v1.json
  - schemas/m057-fd-table-similarity.v1.json
  - schemas/m058-plotextractor-figure-caption.v1.json
key_decisions:
  - Keep schemas draft-07 and permissive with additionalProperties true.
  - Validate historical GROBID diagnostics without modifying M050-M058 artifacts.
duration: 
verification_result: passed
completed_at: 2026-06-12T10:17:23.855Z
blocker_discovered: false
---

# T01: Created six permissive draft-07 JSON schemas for manifest-driven PDF ingest and parser outputs.

**Created six permissive draft-07 JSON schemas for manifest-driven PDF ingest and parser outputs.**

## What Happened

Created `schemas/` with the batch manifest schema, parser operation envelope, GROBID diagnostic/TEI schema, OpenDataLoader schema, M057 table similarity schema, and M058 PlotExtractor figure-caption schema. Each schema includes `$id`, `$schema`, title, description, permissive additional properties, examples, and explicit false safety semantics where applicable. The GROBID schema accepts both the target TEI-adapter shape and historical daily-archive diagnostic outputs so retroactive validation can operate on existing artifacts without mutating M050-M058 files.

## Verification

Verified all six schemas with jsonschema Draft7Validator.check_schema and `uv run pytest tests/test_m059_s01.py -q` (8 passed).

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python - <<'PY'
import json
from pathlib import Path
from jsonschema import Draft7Validator
for path in sorted(Path('schemas').glob('*.json')):
    schema=json.loads(path.read_text())
    Draft7Validator.check_schema(schema)
    print(path, 'ok')
PY` | 0 | ✅ pass | 2700ms |
| 2 | `uv run pytest tests/test_m059_s01.py -q` | 0 | ✅ pass | 9100ms |

## Deviations

GROBID schema includes a compatibility branch for historical diagnostic outputs because existing M055 GROBID artifacts are not TEI-adapter JSON.

## Known Issues

None.

## Files Created/Modified

- `schemas/daily-archive.pdf-batch-manifest.v1.json`
- `schemas/daily-archive.parser-op.v1.json`
- `schemas/grobid-tei.v1.json`
- `schemas/opendataloader-pdf.v1.json`
- `schemas/m057-fd-table-similarity.v1.json`
- `schemas/m058-plotextractor-figure-caption.v1.json`
