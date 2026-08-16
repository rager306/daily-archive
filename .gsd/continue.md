# Continue — M399-a2hxn7 complete

## Last action

M399-a2hxn7 “ACAP runtime guards and promotion control” is fully complete and validated (`verdict: pass`). All four slices and 13/13 tasks are closed:

- S01 runtime ACAP admission guards;
- S02 unified candidate promotion protocol;
- S03 generic durable scheduler with fenced recovery;
- S04 same-process control-plane evidence and Governor hardening.

PIR-014, PIR-015, and PIR-016 are resolved. R093 is validated. Final milestone completion commit: `3d0c86f7`. D127 remains locked and optimizer auto-promotion remains disabled.

## Next action

No active milestone. The next recommended milestone is the previously researched **revision-grounded multimodal candidate evidence** wave for R094 / PIR-018, PIR-019, and PIR-020:

- revision-scoped Figure/Table/Equation candidate evidence;
- TEI table-cell and equation formula provenance;
- no synthetic visual structure, no graph-truth promotion, and no import unlock.

Create a new milestone ID with `gsd_milestone_generate_id`, save research, plan S01, then continue auto mode.

## Verification snapshot

- Final M399 source revision: `sha256:3560a3e47e7c5b8d6992aa30343ca2cb4b3ff7c978c6dd7171349055e3955928`.
- S04 control-plane UAT: PASS (`acap_control_plane_test` 3/3; integrated ledger; Governor YAML 59 rules / 12 dimensions; self-test 44/44 including 7 ACAP cases; active phase violations=0).
- S03 scheduler UAT: PASS (integration recovery 11/11; domain scheduler 31/31; runner unit 10/10; focused D127 crash-recovery case 1/1).
- S02 promotion UAT: PASS (domain promotion 25/25; application promotion 14/14; shared-surface contract 8/8; Governor 37/37).
- Governor active has no blocking reverse-ADR violation; only medium `resource_target_bloat` build-cache guidance remains.

## Do not

Hand-edit `.gsd` planning artifacts or `gsd.db`. Unlock D127 without explicit user approval. Enable optimizer/deployment auto-promotion. Run workspace-wide cargo tests (rocksdb trap). Treat the target-size warning as a product failure.
