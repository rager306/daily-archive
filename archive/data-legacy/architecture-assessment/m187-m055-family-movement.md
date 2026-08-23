# M187 M055 Family Movement

## Verdict

**PASS: M055 and M055deep manifest writes now use the application atomic manifest writer.**

## Changes

| File | Function | Change |
|---|---|---|
| `scripts/benchmark_m055_corpus_manifest.py` | `build_corpus_manifest` | Replaced direct `output_path.write_text(...)` with `write_manifest_json_atomic(output_path, payload, sort_keys=True)`. |
| `scripts/build_m055deep_corpus_manifest_20.py` | `write_manifest` | Replaced direct `output_path.write_text(...)` with `write_manifest_json_atomic(output_path, stable_payload, sort_keys=True)`. |

## Preserved behavior

- JSON shape remains unchanged.
- Sorted-key output remains enabled.
- Parent directory creation is handled by the atomic writer.
- `build_corpus_manifest` still returns the in-memory payload.
- `write_manifest` still returns `stable_payload`.
- M055deep still preserves existing `generated_at` when stable content is unchanged.

## Focused behavior proof

- M055 corpus manifest focused tests passed: `3 passed, 8 deselected`.
- M055deep corpus tests passed: `6 passed`.

## Direct write check

Direct `output_path.write_text` calls are absent from the two S02 target scripts after movement. Both scripts import and call `write_manifest_json_atomic`.
