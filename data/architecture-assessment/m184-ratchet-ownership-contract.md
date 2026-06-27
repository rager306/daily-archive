# M184 Script Only Ratchet and Ownership Contract

## Verdict

**Ratchet status: active for M184.**

The canonical write-path baseline starts M184 at:

```text
total_records=341
script-only=89
unknown=0
shared-state=0
```

## Executable ratchet

`tests/test_inventory_write_paths.py` must fail if the canonical baseline regresses above:

```text
script-only <= 4
unknown == 0
shared-state == 0
```

The ratchet started M184 at `script-only <= 89`, was lowered to `<= 79` after S03, to `<= 55` after S04, to `<= 47` after S05, to `<= 45` after S06, to `<= 33` after S07, and to `<= 4` after S08. Future exact waves may lower the threshold again after canonical refresh. They must never raise it without an explicit GSD decision and generated delta proof.

## Category ownership expectations

| Category style | Expected owner | Movement rule |
|---|---|---|
| `script-only` | process-boundary script | reduce only by exact source-path review or leave with reason |
| `*-output` script categories | milestone or benchmark artifact producer | exact source paths only |
| `caller-owned` | caller or adapter boundary | no movement without caller contract proof |
| `run-scoped` | run-local artifact writer | preserve locality and cleanup assumptions |
| `append-log` | diagnostics/event log owner | preserve append semantics |
| `shared-state` | stable shared index/cache/catalog owner | must remain zero unless explicitly accepted |
| cache/index/manifest candidates | lifecycle owner | no movement without owner, invalidation, consumer, concurrency proof |

## Movement protocol

Every M184 movement slice must follow ADR-035:

1. baseline before edits;
2. exact candidate audit;
3. GitNexus impact before symbol edits;
4. source-path scanner rule only;
5. generated delta;
6. canonical refresh;
7. strict drift pass;
8. focused tests and quality stack.

## Stop conditions

- No broad prefix, target-name, cache, index, manifest, markdown, or converter rule.
- No treating GitNexus UNKNOWN as proof.
- No cache/index/manifest movement without lifecycle proof.
- No direct extractor-to-graph write.
