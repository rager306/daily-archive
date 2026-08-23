# M181 Canonical Docs Cleanup Scope

## Decision

**No docs/CI cleanup edits are selected.**

Reason: active CI is already canonical-only, and the remaining matches are either canonical references or GSD historical/append-only projection content. Editing `.gsd/DECISIONS.md` or `.gsd/ROADMAP.md` to remove historical milestone references would violate the append-only/current-projection conventions and would not improve active CI behavior.

## S07 action

S07 should close this direction as a no-op with strict canonical-only drift verification:

```text
canonical baseline required: data/architecture-assessment/write-path-inventory-canonical.json
no M179 preview fallback
no M180 preview fallback
strict drift must pass after scanner movement and before canonical refresh decision
```

If strict drift fails only because M181 scanner categories changed, S07 should record expected fail-closed behavior and S10/Sfinal baseline refresh should update canonical after all movement is complete.
