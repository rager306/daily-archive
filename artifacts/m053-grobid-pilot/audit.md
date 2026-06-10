# M053 GROBID Pilot Audit

**Schema version:** `m053-grobid-pilot-audit.v1`
**Generated at:** 2026-06-10T10:09:59.064256+00:00

## Inputs

- Summary: `artifacts/m053-grobid-pilot/summary.json`
- Per-PDF directory: `artifacts/m053-grobid-pilot`
- Total PDFs: 5
- Source schema version: `m053-grobid-pilot.v1`

## Status counts

| status | count |
| --- | ---: |
| success | 0 |
| low_quality_source | 0 |
| blocked | 0 |
| grobid_unavailable | 5 |
| network_error | 0 |
| timeout | 0 |

## Per-PDF packets

| arxiv_id | status | tei_size_bytes | ref_count | body_element_count | m022_repair_candidate | attempts | error |
| --- | --- | ---: | ---: | ---: | --- | ---: | --- |
| `1804.02767` | grobid_unavailable | 0 | 0 | 0 | true | 0 | dry_run_skipped_grobid_call |
| `2108.12409` | grobid_unavailable | 0 | 0 | 0 | true | 0 | dry_run_skipped_grobid_call |
| `2109.10862` | grobid_unavailable | 0 | 0 | 0 | true | 0 | dry_run_skipped_grobid_call |
| `2111.00396` | grobid_unavailable | 0 | 0 | 0 | true | 0 | dry_run_skipped_grobid_call |
| `2203.14465` | grobid_unavailable | 0 | 0 | 0 | true | 0 | dry_run_skipped_grobid_call |

## Safety defaults

Production import is not authorized by this audit; all safety defaults remain false.

```json
{
  "graph_import_allowed": false,
  "graphdb_written": false,
  "import_eligible": false,
  "ladybugdb_written": false,
  "production_import_attempted": false
}
```

## M022 chunk repair candidates

The following PDFs are M022 chunk repair candidates because the GROBID pilot did not produce a usable TEI body/reference surface:

- `1804.02767` — status `grobid_unavailable`
- `2108.12409` — status `grobid_unavailable`
- `2109.10862` — status `grobid_unavailable`
- `2111.00396` — status `grobid_unavailable`
- `2203.14465` — status `grobid_unavailable`
