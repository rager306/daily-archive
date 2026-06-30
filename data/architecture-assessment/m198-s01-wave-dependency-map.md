# M198 S01 Wave Dependency Map

## Verdict

**PASS: M198 has an ordered dependency map from readiness contracts through closeout.** The map keeps high-risk queue/smoke/backend transitions behind contracts, audits, and governance ratchets.

## Wave map

| Wave | Slices | Purpose | Consumes | Produces |
|---|---|---|---|---|
| 1 | S01-S02 | Define risk and evidence contracts | M197 closeout evidence, GitNexus seam map | readiness evidence contract and risk matrix |
| 2 | S03-S06 | Build source-specific baselines | S02 contract | dry-run probe, sync parity baseline, smoke boundary baseline, graph readiness map |
| 3 | S07-S10 | Compare and report readiness | S03-S06 baselines | drift classifier, evidence index, diagnostics, readiness report |
| 4 | S11-S12 | Ratchet blocked transitions | S06-S10 report evidence | governance ratchets and GitNexus impact gates |
| 5 | S13-S15 | Realistic rehearsal and safety checks | S10-S12 gates | realistic readiness rehearsal, smoke parity audit, disabled backend safety evidence |
| 6 | S16-S18 | Final package and closeout | S13-S15 evidence | validation package, operator runbook, requirement outcomes, milestone validation |

## Slice dependency notes

- S02 consumes S01 seam inventory and risk matrix.
- S03 and S04 can proceed in parallel after S02 because dry-run and sync rehearsal are read-only inputs.
- S05 and S06 can proceed after S01/S02 because smoke and graph-readiness surfaces are compatibility inputs only.
- S07 requires S03 and S04 because drift classification compares dry-run and sync rehearsal.
- S08 requires S03, S04, and S07 because the evidence index needs source evidence plus drift status.
- S09 and S10 require S07/S08 because diagnostics and reports consume classified evidence.
- S11 requires S06 and S10 to ratchet graph-readiness and report non-goals.
- S12 requires S11 to record impact gates after governance boundaries are explicit.
- S13 requires S10 and S11 for realistic no-write rehearsal.
- S14 requires S05 and S13 to compare smoke boundary evidence with readiness reports.
- S15 requires S06 and S13 to assert disabled backend safety in realistic context.
- S16 requires S14 and S15 for final validation package.
- S17 requires S16 for operator runbook.
- S18 requires S17 for final closeout readiness.

## S02 required inputs

S02 must consume:

- `data/architecture-assessment/m198-s01-readiness-seam-inventory.md`
- `data/architecture-assessment/m198-s01-impact-risk-matrix.md`
- M197 reactive event contract and operator handoff
- R076, R077, R078

## Non-negotiable boundaries

- No production graph import.
- No schema migration.
- No queue dependency semantic edits.
- No smoke/rehearsal semantic edits.
- No retired graph readiness shim restoration.
- No raw payload persistence.
