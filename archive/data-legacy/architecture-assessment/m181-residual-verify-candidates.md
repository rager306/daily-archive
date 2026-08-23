# M181 Residual Verify Candidates

## Source

Baseline: `data/architecture-assessment/m181-write-path-inventory-baseline.json`

## Candidate groups

| Group | Records | Exact paths | Decision |
|---|---:|---|---|
| `verify_m029` | 8 | 5 paths | Select |
| `verify_m027` | 4 | 2 paths | Select |
| `replay_m031` | 3 | 1 path | Defer: replay family, not verify family |
| `build_m028` | 4 | 2 paths | Defer: builder family, not verify family |

## Selected exact paths

### verify_m029: 8 records

| Path | Records | Targets |
|---|---:|---|
| `scripts/verify_m029_post_validation_remediation.py` | 1 | `path` |
| `scripts/verify_m029_unified_conversion_quality_boundary.py` | 1 | `fd` |
| `scripts/verify_m029_unified_readiness.py` | 1 | `path` |
| `scripts/verify_m029_unified_source_acquisition.py` | 4 | `args.write_report`, `path` |
| `scripts/verify_m029_validation_remediation.py` | 1 | `path` |

### verify_m027: 4 records

| Path | Records | Targets |
|---|---:|---|
| `scripts/verify_m027_mixed_source_catalog.py` | 1 | `REPORT_PATH` |
| `scripts/verify_m027_source_acquisition_boundary.py` | 3 | `args.report`, `path` |

## Deferred exact groups

| Group | Records | Reason |
|---|---:|---|
| `replay_m031` | 3 | Related to replay outputs; should get a separate replay category contract. |
| `build_m028` | 4 | Builder output category, not verify output category. |

## Rejected approaches

- No broad `verify_m029*` or `verify_m027*` prefix matching.
- No generic `verify_*` prefix matching.
- No target-name rules such as `path`, `fd`, `args.write_report`, or `args.report`.
- No cache, markdown, manifest, converter, or index rules.
