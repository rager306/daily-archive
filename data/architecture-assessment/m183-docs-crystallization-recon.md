# M183 Docs Crystallization Recon

## Verdict

**Active ADR crystallization is safe.**

The current active architecture docs describe onion layering and package boundaries but do not yet provide a single accepted ADR for the write-path governance rules that have stabilized across M169-M183. A new project-level ADR is the least invasive way to make the rules discoverable without rewriting GSD history.

## Reviewed active docs

- `doc/adr/ADR-INDEX.md`
- `doc/adr/ADR-034-hexagonal-onion-overlay.md`
- `doc/onion-layers.md`

## Selected edits

1. Create `doc/adr/ADR-035-write-path-governance-and-canonical-baseline.md`.
2. Update `doc/adr/ADR-INDEX.md` project-level count from 34 to 35.
3. Add ADR-035 row after ADR-034.

## ADR-035 reader and action

Reader: future internal engineer changing scanner categories, inventory CI, scripts, or cache/index writes.

Post-read action: safely classify a new write path or update the canonical baseline without broad rules or hidden drift.

## Required content

- Exact source-path scanner policy.
- Canonical baseline update protocol.
- Generated delta requirement.
- Script boundary contract.
- Cache/index/manifest proof gate.
- Guardrails preserved across all future waves.

## Non-goals

- Do not rewrite `.gsd/DECISIONS.md` or `.gsd/ROADMAP.md`.
- Do not change onion layering rules in ADR-034.
- Do not add a broad cache or script category.
