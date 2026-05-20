# S02: Preflight and bounded top up source quota — UAT

**Milestone:** M010-06v9ke
**Written:** 2026-05-20T07:22:54.697Z

# S02: Preflight and bounded top up source quota — UAT

## Expected

- Init/preflight selected M010 manifest.
- Acquire missing Markdown boundedly.
- If underfilled, top up with deterministic replacements.
- Materialize and preflight replacements before scan.

## Result

- Initial readiness: 0/10.
- Original bounded acquisition readiness: 8/10.
- Original failed IDs: `2001.00575v1`, `2001.00817v1`.
- Replacement candidates attempted: 20.
- Replacement Markdown acquired: 16.
- Materialized replacement IDs: `2002.05505v6`, `2405.08246v1`.
- Final materialized readiness: 10/10.
- Final quota shortage: 0.
- Scan allowed: true.
- PDF present: 0/10.
- Production import attempted: false.
- LadybugDB written: false.

## S03 input

Use `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-ready-batch-state.json`.
