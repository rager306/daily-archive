# M187 M058 and M059 Transition Verification

## Verdict

**PASS: S03 completed residual movement and reached the intended `script-only=0` transition state.**

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Focused M058 and M059 tests | PASS: M058 `1 passed`; M059 `8 passed` | `gsd_exec[b0f16811-a478-49ad-a764-432727cab10d]` |
| Manifest contract tests | PASS: 9 passed | `gsd_exec[6997f757-d917-4fdb-8fae-5c3522c9ab19]` |
| Ruff touched files | PASS | `gsd_exec[c4b3d142-8e06-4de6-ad92-af90af455196]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[e8f41d47-bb6f-40d8-b777-0c6d8d557824]` |
| Inventory tests | PASS: 38 passed | `gsd_exec[8ce40df8-0e02-4c88-ba1d-706245ce7f52]` |
| Strict transition delta | PASS: `script-only=0`, `unknown=0`, `shared-state=0`, total delta `-4` vs old baseline | `gsd_exec[3c04eece-bc74-418a-88e3-51922ba3be8a]` |
| GitNexus detect_changes | PASS: LOW, no affected processes | S03 tool output |

## Delta explanation

The old canonical baseline still expects the M186 preserve-ratchet state `script-only=4`. After S02 and S03, all four manifest residual writers delegate to the application atomic writer. Therefore `script-only=0` and total delta `-4` are the intended transition-ratchet result, not a failure.

S04 must now update the canonical inventory baseline to make this transition durable and restore strict drift pass against the new baseline.

## Data repair note

M059 full tests exposed a stale artifact path for `2507.19457` after M186 removed the duplicate `cs-lg` catalog record. S03 repaired the source path to the canonical `cs-cl/.../source/original.pdf` record and added a narrow `find_pdf` fallback for canonical catalog PDFs whose filename is not `{arxiv_id}.pdf`.

## Scope guard

No broad write-path classification rules were introduced. The movement remains limited to the four intended manifest residual writer paths plus the narrow M059 PDF lookup compatibility repair.
