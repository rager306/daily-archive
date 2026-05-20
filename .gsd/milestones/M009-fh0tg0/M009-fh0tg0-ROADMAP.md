# M009-fh0tg0: Validation CLI Provenance and Top Up Hardening

**Vision:** Make validation-batch artifacts auditable and shortage-safe before running any more +10 batches.

## Success Criteria

- Validation CLI outputs can be tied to concrete run logs and hashes.
- Freshness verifier detects stale or mismatched artifacts.
- Scan lineage metadata uses active milestone/batch context.
- Underfilled batches are topped up deterministically within bounds or explicitly blocked.
- A review recommendation gates the next +10 batch.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After this slice, a validation-batch run can emit a commit-safe provenance log entry tying command execution to input and output hashes.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: After this slice, a verifier can prove whether an artifact set matches a recorded CLI run or fail on stale/mismatched outputs.

- [x] **S03: S03** `risk:medium` `depends:[]`
  > After this: After this slice, validation scan summaries identify M009/M008-style active milestone and batch context instead of stale M006 metadata.

- [x] **S04: S04** `risk:high` `depends:[]`
  > After this: After this slice, an underfilled validation batch deterministically draws replacements up to a bounded limit or blocks scan with a shortage report.

- [x] **S05: S05** `risk:medium` `depends:[]`
  > After this: After this slice, review says whether provenance and top-up hardening are sufficient to run the next reviewed +10 batch.

## Boundary Map

| Boundary | In scope | Out of scope |
|---|---|---|
| CLI provenance | Run logs, hashes, timestamps, git commit, command argv, redacted stdout/stderr paths | Raw paper/chunk text in logs |
| Artifact verification | Freshness/hash/lineage checks for produced artifacts | Semantic correctness of KG facts |
| Metadata cleanup | Active milestone/batch context in scan artifacts | Retrofitting all old historical artifacts |
| Quota top-up | Bounded replacement loop when accepted_ready_count < target_count | Unattended run-to-100 or unbounded acquisition |
| Import gates | Preserve no-write/no-import boundaries | Positive KG import or production LadybugDB writes |
