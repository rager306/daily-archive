# M182 Wave Scope

## Decision

Move exactly 7 script-only records into two source-path categories:

```text
build-m028-output=4
replay-m031-output=3
script-only: 110 -> 103
unknown=0
shared-state=0
total_delta=+0
```

## Selected exact paths

### build_m028: 4 records

| Path | Records | Targets |
|---|---:|---|
| `scripts/build_m028_hermes_digest_projection.py` | 2 | `out_dir / DIGEST_FILENAME`, `out_dir / REPORT_FILENAME` |
| `scripts/build_m028_source_metadata_adapters.py` | 2 | `events_path`, `summary_path` |

### replay_m031: 3 records

| Path | Records | Targets |
|---|---:|---|
| `scripts/replay_m031_import_boundary_rehearsal.py` | 3 | `diagnostics_path_out`, `report_path_out`, `summary_path_out` |

## Category names

```text
build-m028-output
replay-m031-output
```

## Test contract

Focused tests must prove:

1. Each exact selected path maps to its category.
2. Future unlisted paths remain `script-only`:
   - `scripts/build_m028_future_unlisted.py`
   - `scripts/replay_m031_future_unlisted.py`
3. Generic targets like `summary_path`, `events_path`, `out_dir / REPORT_FILENAME`, `diagnostics_path_out`, and `report_path_out` are not category rules.
4. No broad `build_m028`, `replay_m031`, `build_`, `replay_`, target-name, cache, manifest, markdown, converter, or index rule exists.

## Rejected approaches

- No broad prefix matching.
- No generic builder/replay family rule.
- No target-name classification.
- No cache/manifest/index movement.
