# M183 Wave Scope

## Decision

Move exactly 14 script-only records into four exact source-path categories:

```text
benchmark-m055-output=5
benchmark-m055deep-output=3
m066-graphdb-benchmark-output=3
test-architecture-audit-output=3
script-only: 103 -> 89
unknown=0
shared-state=0
total_delta=+0
```

## Selected exact paths

### benchmark_m055: 5 records

| Path | Records | Targets |
|---|---:|---|
| `scripts/benchmark_m055_availability_probe.py` | 1 | `output_path` |
| `scripts/benchmark_m055_grobid_only.py` | 1 | `tmp_path` |
| `scripts/benchmark_m055_hybrid_routing.py` | 1 | `tmp_path` |
| `scripts/benchmark_m055_opendataloader_only.py` | 1 | `tmp_path` |
| `scripts/benchmark_m055_vendor_check.py` | 1 | `output_path` |

### benchmark_m055deep: 3 records

| Path | Records | Targets |
|---|---:|---|
| `scripts/benchmark_m055deep_grobid_fulltext.py` | 1 | `tmp_path` |
| `scripts/benchmark_m055deep_hybrid_routing_20.py` | 1 | `tmp_path` |
| `scripts/benchmark_m055deep_opendataloader_correctness.py` | 1 | `path` |

### m066 graphdb benchmark: 3 records

| Path | Records | Targets |
|---|---:|---|
| `scripts/m066_graphdb_full_benchmark.py` | 3 | `artifact_dir / 'scoring-matrix.md'`, `output`, `report_path` |

### test architecture audit: 3 records

| Path | Records | Targets |
|---|---:|---|
| `scripts/audit_test_architecture.py` | 3 | `json_path`, `markdown_path`, `pilot_path` |

## Exclusions

`benchmark_m055_corpus_manifest.py` is excluded from benchmark movement because it is manifest/cache-like and belongs to S06 cache lifecycle review.

## Test fallback examples

```text
scripts/benchmark_m055_future_unlisted.py
scripts/benchmark_m055deep_future_unlisted.py
scripts/m066_graphdb_future_unlisted.py
scripts/audit_test_future_unlisted.py
```

## Rejected approaches

- No broad `benchmark_m055`, `benchmark_m055deep`, `benchmark_`, `audit_`, or `m066_` prefix rule.
- No target-name classification for `output_path`, `tmp_path`, `path`, `json_path`, `markdown_path`, or `report_path`.
- No cache, manifest, markdown, converter, or index rule.
