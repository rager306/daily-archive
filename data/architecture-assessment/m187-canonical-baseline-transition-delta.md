# M187 Canonical Baseline Transition Delta

## Verdict

**PASS: the intended baseline transition is exactly `script-only=4` to `script-only=0`; `unknown` and `shared-state` remain zero.**

## Old canonical baseline

- total records: 341
- `script-only=4`
- `unknown=0`
- `shared-state=0`

## Current inventory after S02-S03 movement

- total records: 337
- `script-only=0`
- `unknown=0`
- `shared-state=0`

## Intended delta

- total delta: `-4`
- `script-only`: `4 -> 0`
- `unknown`: `0 -> 0`
- `shared-state`: `0 -> 0`

## Interpretation

The four removed inventory records are the intended manifest residual retirements:

- M055 five-PDF corpus manifest writer,
- M055deep 20-PDF corpus manifest writer,
- M058 graph manifest JSON writer,
- M059 retroactive batch manifest writer.

No broad write-path classification rules were introduced. No unrelated category drift is authorized by this transition.

## S04 baseline update policy

S04 may update `write-path-inventory-canonical.json` and `.md` to the current inventory only because S02/S03 behavior proof passed and the delta is exactly the planned residual retirement.
