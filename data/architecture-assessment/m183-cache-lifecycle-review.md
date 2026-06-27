# M183 Cache Lifecycle Review

## Verdict

**Cache lifecycle movement: no-move.**

M183 intentionally excluded `scripts/benchmark_m055_corpus_manifest.py` from benchmark movement because it is manifest/cache-like. The post-movement cache-like residual set still lacks exact stable shared cache ownership, invalidation, consumer, and concurrency proof.

## Candidate assessment

| Path | Target | Finding | Decision |
|---|---|---|---|
| `scripts/benchmark_m055_corpus_manifest.py` | `output_path` | Benchmark corpus manifest output; no shared cache lifecycle or invalidation proof | No move |
| `scripts/build_m055deep_corpus_manifest_20.py` | `output_path` | Corpus manifest builder output; no stable shared cache lifecycle or concurrency proof | No move |
| `scripts/m058_build_graph_manifest.py` | `path` | Historical graph manifest output; target too generic and no lifecycle owner | No move |
| `scripts/m059_build_manifest.py` | `actual_output` | Historical manifest output; no invalidation or consumer contract | No move |

## Proof gate from ADR-035

Movement requires exact source ownership, stable lifecycle owner, invalidation semantics, consumer contract, and concurrency/write coordination behavior. M183 has none for these records.

## Boundary

- No broad `cache`, `index`, or `manifest` scanner rule.
- No target-name rule for `output_path`, `path`, or `actual_output`.
- No scanner edit in the cache direction.
