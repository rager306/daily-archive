# M033 OpenDataLoader Adaptix Adapter Probe

## Verdict

`adaptix-adapter-candidate` if all rows mapped; otherwise `needs-attention`.

This report is review-only. It does not claim graph readiness, production import eligibility, or LadybugDB write readiness.

## Per-paper mapping

| Article | Status | Top-level elements | Raw objects | Headings | Tables | Figures/Captions |
|---|---|---:|---:|---:|---:|---:|
| 2507.19457 | `mapped_candidate_only` | 5912 | 6570 | 109 | 246 | 38 |
| 2512.24601 | `mapped_candidate_only` | 798 | 1184 | 55 | 215 | 46 |
| 2605.26525v1 | `mapped_candidate_only` | 894 | 1299 | 57 | 198 | 64 |

## Diagnostics

- `info` `adaptix_mapping_succeeded` 2507.19457: OpenDataLoader JSON loaded into typed Adaptix model and candidate summary was generated.
- `info` `adaptix_mapping_succeeded` 2512.24601: OpenDataLoader JSON loaded into typed Adaptix model and candidate summary was generated.
- `info` `adaptix_mapping_succeeded` 2605.26525v1: OpenDataLoader JSON loaded into typed Adaptix model and candidate summary was generated.

## Safety

- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `production_import_attempted=false`
- `import_eligible=false`
