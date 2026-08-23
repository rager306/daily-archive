# M187 Final Validation Evidence

## Verdict

**PASS: final representative gates are green with `script-only=0`.**

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Inventory, test architecture, and onion tests | PASS: 56 passed | `gsd_exec[af63c86e-829e-4e39-9dc9-33e240e25cee]` |
| Catalog plus manifest tests | PASS: 22 passed | `gsd_exec[c4c58e60-d422-40bf-9785-57e0b1a0876e]` |
| Residual focused tests | PASS: M055 3 passed/8 deselected, M055deep 6 passed, M058 1 passed, M059 8 passed | `gsd_exec[3a9fdb3f-14c0-4e36-a553-1e3a20afbcb1]` |
| Article catalog verifier plus M030 validate-only | PASS | `gsd_exec[bed7f8b7-bc74-4d9c-ab5c-3feedeb6385d]` |
| Ruff touched files | PASS | `gsd_exec[10475c1e-477d-4297-a571-8a212b96c37c]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[bb05cdb0-a76d-4694-970c-2a2a96c67a39]` |
| Strict drift final | PASS: `script-only=0`, `unknown=0`, `shared-state=0`, total delta `+0` | `gsd_exec[0edf76cf-c610-4672-9429-2fa806b3fefe]` |
| GitNexus detect_changes | PASS: LOW, no affected processes | S05 tool output |

## Final state

M187 retired all four manifest script-only residuals, updated the canonical baseline, and preserved `unknown=0` and `shared-state=0`. The final canonical inventory has total records 337, with `scripts=257` and `src=80`.
