# Final recommendation — M008 first new +10 validation batch

## Recommendation

Close M008 as a successful first new +10 operational validation batch, but do **not** run another +10 batch until bounded top-up automation is implemented or equivalently proven.

## Why M008 can close

M008 produced useful, quota-gated operational evidence:

- S01 selected a genuinely new +10 corpus with no M006 overlap.
- S02 initialized and preflighted the batch, then acquired missing Markdown through bounded fast-only acquisition.
- S03 added a quota-fill gate before scan.
- The quota gate proved:
  - `target_count=10`
  - `attempted_count=10`
  - `accepted_ready_count=10`
  - `rejected_count=0`
  - `shortage_count=0`
  - `scan_allowed=true`
- The scan produced:
  - `paper_count=10`
  - `chunk_count=1591`
  - `outlier_count=6`
  - `import_eligible_chunk_count=0`

The current scan stayed within safety boundaries:

- positive KG import remains blocked;
- production LadybugDB writes remain blocked;
- raw paper text remains excluded from JSON/JSONL artifacts;
- chunk text remains excluded;
- embeddings and vectors remain excluded;
- optimizer traces remain excluded.

## Why the next +10 should wait

Independent review returned `FLAG` because the new quota-fill gate proves only the current happy path. It does not yet implement or demonstrate what happens when the initial selected batch cannot reach the target quota.

Before another +10, add bounded top-up automation or an equivalent CLI gate that handles shortages:

1. detect `accepted_ready_count < target_count`;
2. reject or defer unready papers with explicit reasons;
3. draw deterministic replacement candidates from the candidate pool;
4. enforce max-attempt or max-candidate bounds;
5. rerun bounded preflight/acquisition for replacements;
6. block scan explicitly if the quota still cannot be filled;
7. write quota-fill summary and diagnostics for both accepted and rejected candidates.

## Additional fix before future scans

The M008 scan summary includes stale traceability metadata:

```json
"milestone": "M006-638rza"
```

This appears to come from the reused thirty-paper scan helper. It does not invalidate the current counts, but future scan artifacts should carry the active milestone/batch ID to avoid confusing M006 and M008 evidence.

## Explicit non-recommendations

Do not treat M008 as proof of trusted KG semantic correctness.

Do not enable positive KG import.

Do not write to production LadybugDB.

Do not start unattended run-to-100 automation.

Do not use MiniMax as orchestrator or source of truth.

## Suggested next milestone

Plan a focused automation hardening milestone before the next validation batch:

- bounded quota top-up CLI workflow;
- shortage-path tests and artifacts;
- active milestone/batch metadata in scan summaries;
- then run the next reviewed +10 batch.
