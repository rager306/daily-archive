# M186 S16 Integrated Verification

## Verdict

**PASS: post-S15 integrated baseline verification is green under preserve-ratchet.**

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Full M027 mixed-source catalog tests | PASS: 13 passed | `gsd_exec[df77f352-b2bd-40f0-bda2-05e9163a8168]` |
| M030 requested-ref intake validate-only | PASS | `gsd_exec[d99cd951-09e9-4188-a98b-7f8daa790035]` |
| Manifest closeout, lifecycle, ratchet, and IO tests | PASS: 12 passed | `gsd_exec[4acf69be-f607-45e3-9387-490e2c4f9e05]` |
| Article catalog verifier | PASS | `gsd_exec[ddce43df-b4cd-4964-9463-d63db0b4c929]` |
| Inventory tests | PASS: 38 passed | `gsd_exec[7b21458b-d18e-46fc-8bbe-dea479101b35]` |
| Test architecture guard tests | PASS: 6 passed | `gsd_exec[e7652764-7931-48ce-9fc0-229c6d906a42]` |
| Onion layering tests and JSON guard | PASS: 12 passed plus guard exit 0 | `gsd_exec[0dd34837-95d4-4d48-a77c-868a206fd8dd]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[9be3e05b-a021-491f-917e-790eaccfb97e]` |
| Strict write-path drift | PASS: `script-only=4`, `unknown=0`, `shared-state=0`, total delta `+0` | `gsd_exec[39de4a4a-0c0a-4a75-ab6c-87720fb91634]` |
| GitNexus detect_changes | PASS with known accumulated M186 MEDIUM scope | S16 tool output |

## Integration result

S08-S14 manifest lifecycle/ratchet contracts and S15 catalog drift remediation coexist without changing the active write-path ratchet. The four manifest residuals remain blocked/no-move under `preserve-ratchet`, and M027/M030 no longer require scoped exclusions for the previously known catalog baseline drift.

## Risk note

GitNexus continues to report MEDIUM risk because the working tree includes accumulated M186 source and test changes from earlier slices. S16 introduced documentation-only assessment artifacts and did not edit source symbols.
