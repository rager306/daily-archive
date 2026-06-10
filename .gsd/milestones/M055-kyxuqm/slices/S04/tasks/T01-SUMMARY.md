---
id: T01
parent: S04
milestone: M055-kyxuqm
key_files:
  - scripts/benchmark_m055deep_grobid_fulltext.py
  - artifacts/m055deep-parser-benchmark/grobid-fulltext-20/summary.json
  - artifacts/m055deep-parser-benchmark/grobid-fulltext-20/per-pdf/*.json
  - artifacts/m055deep-parser-benchmark/grobid-fulltext-20/tei/*.tei.xml
key_decisions:
  - Keep the existing GROBID section_count field and add sections as a non-breaking diagnostic list derived from TEI section heads.
duration: 
verification_result: passed
completed_at: 2026-06-10T12:02:14.563Z
blocker_discovered: false
---

# T01: Re-ran the GROBID fulltext probe across the 20-PDF corpus and emitted 20 per-PDF packets plus aggregate summary.

**Re-ran the GROBID fulltext probe across the 20-PDF corpus and emitted 20 per-PDF packets plus aggregate summary.**

## What Happened

Executed the GROBID /api/processFulltextDocument benchmark against artifacts/m055deep-parser-benchmark/corpus-manifest-20.json with output under artifacts/m055deep-parser-benchmark/grobid-fulltext-20. The run produced 20 per-PDF JSON packets, raw TEI outputs, and summary.json. While validating the S04 packet contract, I found the existing fulltext probe emitted section_count but not the explicit sections field requested by S04, so I added a non-breaking sections list derived from TEI section heads and reran the 20-PDF probe.

## Verification

uv run python scripts/benchmark_m055deep_grobid_fulltext.py --corpus-manifest artifacts/m055deep-parser-benchmark/corpus-manifest-20.json --output-dir artifacts/m055deep-parser-benchmark/grobid-fulltext-20 --grobid-url http://127.0.0.1:8070 exited 0; compact artifact check reported 20 packets, no missing sections, min_sections 11, max_sections 96.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/benchmark_m055deep_grobid_fulltext.py --corpus-manifest artifacts/m055deep-parser-benchmark/corpus-manifest-20.json --output-dir artifacts/m055deep-parser-benchmark/grobid-fulltext-20 --grobid-url http://127.0.0.1:8070` | 0 | ✅ pass | 69100ms |
| 2 | `uv run python - <<'PY'
import json
from pathlib import Path
packets=sorted(Path('artifacts/m055deep-parser-benchmark/grobid-fulltext-20/per-pdf').glob('*.json'))
missing=[]
section_counts=[]
for path in packets:
    p=json.loads(path.read_text())
    if 'sections' not in p:
        missing.append(path.name)
    section_counts.append(len(p.get('sections', [])))
print('grobid_packets', len(packets), 'missing_sections', missing, 'min_sections', min(section_counts), 'max_sections', max(section_counts))
PY` | 0 | ✅ pass | 1000ms |

## Deviations

Added the required GROBID per-PDF sections field before finalizing artifacts; existing fields and aggregate counts were preserved.

## Known Issues

None.

## Files Created/Modified

- `scripts/benchmark_m055deep_grobid_fulltext.py`
- `artifacts/m055deep-parser-benchmark/grobid-fulltext-20/summary.json`
- `artifacts/m055deep-parser-benchmark/grobid-fulltext-20/per-pdf/*.json`
- `artifacts/m055deep-parser-benchmark/grobid-fulltext-20/tei/*.tei.xml`
