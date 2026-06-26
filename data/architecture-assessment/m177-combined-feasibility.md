# M177 Combined Feasibility

## Verdict

**Feasible with bounded slices.** All five requested directions can be handled in one GSD milestone because each is separable by exact source path family, has its own decision and verification slice, and can be aggregated through the scanner generated delta at the end.

## Included directions

1. **R024 script inventory wave**: review corpus build, replay, probe, coverage, and quality scripts as exact source families.
2. **Scanner self-output ownership review**: review `scripts/inventory_write_paths.py` writes without changing generated delta semantics.
3. **Markdown cache policy review**: review markdown converter output/cache writes, preserving cache-like paths unless exact ownership is proven.
4. **Queue and smoke output ownership review**: review queue and smoke outputs without weakening conservative queue/state signals.
5. **Inventory delta CI wiring**: add a bounded CI drift check only if it avoids committing generated CI artifacts and remains cheap.

## Why combined is safe

- The milestone keeps one baseline and one final generated delta.
- Each implementation slice is independently testable.
- Exact source-path rules remain the only allowed scanner classification mechanism.
- Broad target names such as `output_path`, `summary_path`, `cache_path`, `destination`, and `delta_path` remain forbidden as classification keys.
- Mixed, queue-like, cache-like, and unreviewed paths remain conservative until exact review.

## Stop conditions

- If a direction cannot be classified by exact source path, it becomes an explicit no-move artifact rather than a broad rule.
- If CI wiring would generate noisy artifacts or duplicate local evidence, it is deferred with a smoke-checked local command instead.
- If final inventory would produce `unknown>0` or `shared-state>0`, the milestone stops before closeout.

## Baseline

```text
total_records=341
script-only=235
unknown=0
shared-state=0
```
