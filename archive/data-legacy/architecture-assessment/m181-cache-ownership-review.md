# M181 Cache Ownership Review

## Verdict

**Cache lifecycle movement: no-move.**

The cache-like residual set contains four script-only manifest/index flavored outputs, not proven stable shared cache lifecycle ownership. No exact cache category is safe to add in M181.

## Candidate assessment

| Path | Target | Finding | Decision |
|---|---|---|---|
| `scripts/benchmark_m055_corpus_manifest.py` | `output_path` | Caller-provided benchmark manifest output; no shared cache lifecycle or invalidation proof | No move |
| `scripts/build_m055deep_corpus_manifest_20.py` | `output_path` | Caller-provided corpus manifest output; no shared cache lifecycle or concurrency proof | No move |
| `scripts/m058_build_graph_manifest.py` | `path` | Historical graph manifest output; target name too generic and no lifecycle ownership proof | No move |
| `scripts/m059_build_manifest.py` | `actual_output` | Historical manifest output; no stable cache lifecycle or invalidation contract | No move |

## Already-reviewed context

Existing cache-like categories remain precise and should not be broadened:

- `caller-owned`
- `caller-owned-index`
- `parser-replay-output`

## Proof gaps

Movement would require all of the following, and M181 does not have them:

1. Exact source ownership of a stable shared cache/index lifecycle.
2. Explicit invalidation semantics.
3. Concurrency/write coordination behavior.
4. Consumer contract proving the path is not just caller-owned output.

## Boundary

- No broad `cache`, `index`, `manifest`, `markdown`, or converter rule.
- No target-name rule for `output_path`, `path`, or `actual_output`.
- No scanner edit in the cache direction.
