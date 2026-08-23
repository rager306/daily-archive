# M170 CLI Cache Coordination Result

## Verdict

**CLI cache coordination closes as policy-only under the S05 atomic-only decision.**

No lock-file or compare-and-swap code was added for CLI per-paper JSON artifacts in M170.

## Why no CLI code change is needed now

The current CLI per-paper stable artifacts already use same-directory temp plus atomic replace:

- `src/research_graph/cli/__init__.py::_write_text_atomic(...)`
- `src/research_graph/cli/__init__.py::write_paper_artifacts(...)`

This prevents partially written final `paper.json` or `scored.json` files. Same-key concurrent writers remain last-writer-wins, but current M170 scope has no activation path requiring exactly-once cache population or stale-overwrite detection.

## Verification

Focused CLI test:

```text
uv run pytest tests/test_analysis.py::test_s05_subprocess_same_date_rerun_overwrites_stable_paths -q
```

Result:

```text
1 passed
```

Evidence: `gsd_exec[7cf38708-a89f-4de4-9d90-fb455795c682]`.

## Residual risk

Atomic-only does not prevent duplicate same-key CLI work or last-writer-wins when two different scoring payloads race for the same paper id. This remains acceptable until a real multi-worker CLI artifact writer activation path appears.

## Future trigger

Add CLI lock/CAS work only if:

1. multiple worker processes write the same paper id concurrently with non-deterministic scoring inputs;
2. cache consumers require stale-overwrite detection;
3. a generation id or hash authority becomes part of the CLI artifact contract.
