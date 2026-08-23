# M187 M058 and M059 Movement

## Verdict

**PASS: M058 and M059 manifest writes now use the application atomic manifest writer.**

## Changes

| File | Function | Change |
|---|---|---|
| `scripts/m058_build_graph_manifest.py` | `write_json` | Replaced direct `path.write_text(...)` with `write_manifest_json_atomic(path, payload, sort_keys=True)`. |
| `scripts/m059_build_manifest.py` | `finalize_manifest` | Replaced direct `actual_output.write_text(...)` with `write_manifest_json_atomic(actual_output, manifest, sort_keys=True)`. |
| `scripts/m059_build_manifest.py` | `find_pdf` | Added a narrow fallback from `source/{arxiv_id}.pdf` to `source/*.pdf` for canonical catalog records that store the PDF as `original.pdf`. |
| `artifacts/m056-bfs-graph/cumulative-corpus.json` | data | Repaired stale `2507.19457` path from removed duplicate `cs-lg` record to canonical `cs-cl/.../source/original.pdf`. |

## Preserved behavior

- M058 combined graph manifest output remains stable and idempotent.
- M059 `finalize_manifest` still resolves relative output paths under the repository root and returns the manifest object.
- M059 six direct builders remain centralized through `finalize_manifest`.
- Safety defaults remain explicit false.

## Focused behavior proof

- M058 focused graph manifest test passed: `1 passed`.
- M059 full tests passed after completed regeneration: `8 passed`.
- M059 manifests were regenerated with the atomic writer.
- Direct `path.write_text`/`actual_output.write_text` writer calls are absent from the S03 target writer paths.

## Deviation note

M059 full tests exposed stale artifact data from the prior duplicate `2507.19457` catalog record removal. The repair was data-consistency work plus a narrow `find_pdf` fallback for canonical `original.pdf` source files, not a broad write-path classification change.
