# M177 Scope Decision

## Decision

M177 will include all five requested directions in one bounded milestone. Scanner category movement is limited to exact source-path rules for reviewed R024 scripts, scanner self-output, and queue/smoke scripts. Markdown cache policy and CI wiring are included as review/verification slices, but they do not require broad scanner reclassification.

## Allowed scanner movement

1. **R024 exact script families**: 23 records from selected `scripts/*r024*.py` paths.
2. **Scanner self-output**: 3 records from `scripts/inventory_write_paths.py`.
3. **Queue/smoke exact scripts**: 11 records from selected queue/smoke script paths.

## No-move policy

- `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` remains `caller-owned` in the scanner; M177 documents cache policy rather than inventing a cache category from target names.
- Existing workflow records under `src/research_graph/workflows/universal_kb/` remain in their current categories.
- CI wiring is workflow-only and does not become a scanner category.
- Unreviewed scripts remain `script-only`.

## Acceptance target

If all scanner movement lands, M177 should move **37 script-only records** and end near:

```text
script-only=198
unknown=0
shared-state=0
```

The final count must be proven by generated inventory and scanner-generated delta, not hand arithmetic.
