# M054 PDF Acquisition Audit

**Schema version:** `m054-pdf-acquisition-audit.v1`
**Generated at:** 2026-06-10T06:52:15.835333+00:00

## Inputs

- Acquisition log: `artifacts/m054-pdf-acquisition/acquisition-log.json` (sha256:0c7790fc32296004)
- Target subset: `artifacts/m054-pdf-acquisition/target-subset.json` (sha256:572e1e516b5d02ea)
- Records expected: 5
- Records acquired: 5 (100.0%)
- Total bytes acquired: 9.0 MB
- Total bytes logged: 9.0 MB

## Status Counts

| status | count |
| --- | ---: |
| `acquired` | 5 |

## Per-Record Table

| arxiv_id | status | bytes | http | attempts | error |
| --- | --- | ---: | ---: | ---: | --- |
| `1804.02767` | acquired | 2.3 MB | 200 | 1 | — |
| `2108.12409` | acquired | 741.2 KB | 200 | 1 | — |
| `2109.10862` | acquired | 1.9 MB | 200 | 1 | — |
| `2111.00396` | acquired | 3.2 MB | 200 | 1 | — |
| `2203.14465` | acquired | 753.8 KB | 200 | 1 | — |

## Safety Defaults (5-Flag Block)

Per M045 trajectory `prohibited-claim scan` and ADR-006 binding (agent layer is diagnostic-only, no graph writes, no promotion authority):

```json
{
  "graph_import_allowed": false,
  "graphdb_written": false,
  "import_eligible": false,
  "ladybugdb_written": false,
  "production_import_attempted": false
}
```

## Next-Step Recommendation

All 5 target records acquired. The next gate is `M055` (live GROBID/OpenDataLoader/Adaptix pilot) on these PDFs. Per M044 lesson, expect 0-3 of 5 to produce usable conversion output; the rest will fail-closed with `low_quality_source` or `missing_extraction_path` and become candidates for the chunk repair path (M022) or a re-acquisition with an alternative source.

## Audit Trail

- Acquisition log SHA-256 prefix: `0c7790fc32296004` (see `artifacts/m054-pdf-acquisition/acquisition-log.json`)
- Target subset SHA-256 prefix: `572e1e516b5d02ea` (see `artifacts/m054-pdf-acquisition/target-subset.json`)
- Audit script: `scripts/audit_m054_pdf_acquisition.py`
