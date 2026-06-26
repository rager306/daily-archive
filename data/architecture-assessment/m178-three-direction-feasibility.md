# M178 Three Direction Feasibility

## Verdict

**Feasible together with bounded slices.** The requested directions can proceed in one milestone because they are separable: residual script classification changes scanner rules, strict CI drift changes workflow policy, and cache coordination is primarily policy/no-move review unless exact shared cache ownership appears.

## Included directions

1. **Next exact script-only family wave** against residual `script-only=198`.
2. **Stricter inventory delta CI drift policy** upgrading M177 smoke into zero-drift checking against the committed final inventory when present.
3. **Dedicated cache-coordination review** documenting caller-owned versus shared-cache ownership without broad target-name rules.

## Safety boundaries

- Scanner movement must be exact source-path based.
- Generic target names such as `summary_path`, `output_path`, `cache_path`, `path`, and `destination` remain invalid classifier keys by themselves.
- Strict CI must write only temporary generated files and must not commit generated artifacts.
- Cache coordination must not invent a broad cache category unless exact stable shared cache/index ownership is proven.

## Stop conditions

- If candidate script families are mixed, they remain `script-only`.
- If strict CI drift is noisy before final inventory exists, fallback behavior must be explicit and retested after final inventory is generated.
- If cache review finds only caller-owned outputs, the result is no-move policy plus regression tests.

## Baseline

```text
total_records=341
script-only=198
unknown=0
shared-state=0
```
