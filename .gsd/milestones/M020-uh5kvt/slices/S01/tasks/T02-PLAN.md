---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Validate protocol safety guard

Write a protocol guard and validation report proving the schema blocks fact promotion, production import, LadybugDB writes, raw corpus persistence, and MiniMax authority behavior. Validate S01 against guard assertions.

## Inputs

- `.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md`
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`

## Expected Output

- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-guard.json`
- `.gsd/milestones/M020-uh5kvt/slices/S01/protocol-validation-report.md`

## Verification

uv run python inline assertions over protocol guard and schema

## Observability Impact

Records guard evidence for future agents before S02 uses the protocol.
