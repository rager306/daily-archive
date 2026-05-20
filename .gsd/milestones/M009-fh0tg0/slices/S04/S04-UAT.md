# S04: Bounded quota top-up automation — UAT

**Milestone:** M009-fh0tg0
**Written:** 2026-05-20T05:23:53.865Z

# S04: Bounded quota top-up automation — UAT

## Expected

- Underfilled batches can plan deterministic replacements.
- Scan remains blocked if bounded replacement attempts cannot fill quota.
- Top-up reports remain redacted and no-write/no-import.

## Result

Pass sample:

- Initial accepted ready: 1.
- Target count: 3.
- Accepted replacements: 2.
- Final accepted ready: 3.
- Scan allowed: true.

Blocked sample:

- Initial accepted ready: 1.
- Target count: 3.
- Max candidates considered: 1.
- Accepted replacements: 0.
- Remaining shortage: 2.
- Scan allowed: false.
- Blocker count: 1.

Verification:

- 14 focused tests passed.
- Ruff passed.
- Safety flags false.

## Caveat

This is bounded top-up planning, not real replacement acquisition. Future batch execution still needs to wire accepted replacements into acquisition/preflight.
