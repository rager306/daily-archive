# M195 S05 Resume Verification

## Verdict

**PASS: resume and artifact dependency gates fail closed and unblock only on metadata hash match.**

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Artifact registry expected red | PASS: failed before implementation | `gsd_exec[7fae8428-b31b-4093-bf3d-9deee4498111]` |
| Artifact registry targeted tests | PASS: 3 passed, 26 deselected | `gsd_exec[30a7877b-4c0e-4a16-86af-7ea89c765990]` |
| Full queue suite | PASS: 29 passed | `gsd_exec[eb57ec69-18ba-4b00-90ef-e3a4ed1a64af]` |
| Stale, retry, lease, artifact guard tests | PASS: 7 passed, 22 deselected | `gsd_exec[c4fa3f41-dcde-427e-be38-d5f0193cd1ce]` |
| No-write rehearsal compatibility | PASS: 8 passed | `gsd_exec[4415ec42-7d53-4b2b-9cd7-6b5356c4ecef]` |

## Verified behavior

- Artifact dependencies without `expected_hash` remain blocked even if the artifact ref is registered.
- Artifact dependencies with `expected_hash` remain blocked while the registered hash is missing or mismatched.
- Artifact dependencies unblock only after the exact metadata ref/hash pair is registered.
- Artifact registration emits `artifact_registered` queue events for dependent jobs.
- Artifact registration rejects raw refs, secret-shaped refs, empty hashes, and secret-shaped hashes.
- Stale input/tool/contract drift remains fail-closed via `mark_stale`.
- Retryable failure and expired lease resume behavior remains compatible with the new artifact registry.
- No-write rehearsal and substrate rehearsal remain compatible.

## Boundary statement

S05 did not calculate file hashes, inspect artifact payloads, read corpus text, call network/LLM providers, write graph state, or promote import eligibility. The artifact registry is metadata-only and local to queue dependency safety.
