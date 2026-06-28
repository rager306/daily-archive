# M186 M027 and M030 Catalog Drift Verification

## Verdict

**PASS: M027/M030 catalog baseline drift is remediated.**

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Catalog consistency script | PASS | `gsd_exec[f16b8554-3b64-4b68-bac1-2fed9b43e54c]` |
| Full M027 mixed-source catalog tests | PASS: 13 passed | `gsd_exec[582223ad-d477-4789-921b-cde50fdd4eac]` |
| M030 validate-only check | PASS | `gsd_exec[2b41cbe8-b992-4c2a-a638-25ef74ac96a3]` |
| Generic article catalog verifier | PASS | `gsd_exec[fcd633d6-3ff7-4920-9d57-7d9e02131aae]` |
| Inventory tests | PASS: 38 passed | `gsd_exec[f5b9fea7-b229-400d-88c5-f1a15138a070]` |
| Strict write-path drift | PASS: script-only=4, unknown=0, shared-state=0 | `gsd_exec[93767291-4680-4dcb-995e-6cdd91514c9e]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[76eee702-c5d5-40b3-8032-664ff2d21c76]` |
| GitNexus detect_changes | PASS: MEDIUM accumulated M186 scope | S15 tool output |

## Result

Full M027 no longer needs scoped exclusions for the previously known M030 catalog index drift. The repaired catalog remains fail-closed and local-only.
