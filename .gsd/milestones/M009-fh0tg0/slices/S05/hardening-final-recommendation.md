# Final recommendation — M009 validation CLI hardening

## Recommendation

M009 is sufficient to allow **one carefully reviewed next +10 validation batch** only under explicit runbook gates.

M009 is **not** sufficient for unattended batch automation, run-to-100 behavior, positive KG import, or production LadybugDB writes.

## Required gates for the next +10

The next +10 may proceed only if all of these are enforced during execution:

1. The scan command is invoked with the active milestone id:

   ```bash
   validation-batch scan --milestone-id <active-milestone-id>
   ```

2. A provenance JSONL entry is produced for the real run.

3. `validation-batch verify-artifacts` returns `fresh` for that real provenance entry.

4. The provenance entry includes expected artifact metadata for at least:

   ```text
   milestone_id
   batch_id
   ```

5. If initial accepted-ready count is below target, bounded top-up planning must run.

6. Any accepted replacements must be materialized into the batch state and preflighted before scan.

7. `scan_allowed=true` from top-up is treated as planning permission only; final scan still requires source-ready preflight and freshness verification.

8. Positive KG import remains blocked.

9. Production LadybugDB writes remain blocked.

## What M009 proved

M009 proved the hardening primitives and gates needed for a reviewed next batch:

- Provenance entries can record command metadata, git commit, timestamps, input/output hashes, and safety flags without raw content.
- Freshness verifier detects fresh artifacts, mutated outputs, missing outputs, mutated inputs, and invalid provenance.
- Active scan lineage can stamp `milestone_id` and `batch_id` into scan artifacts.
- Lineage verification can fail stale M006-style metadata via `artifact_metadata_mismatch`.
- Bounded top-up planning can produce both:
  - a successful replacement plan, and
  - an explicit shortage blocker.

## What M009 did not fully automate

M009 did not wire automatic provenance emission into `validation-batch init`, `preflight`, or `scan`.

M009 did not implement real acquisition/preflight mutation for accepted top-up replacements.

M009 did not enable unattended scaling.

## Next operational step

Plan the next milestone as a single reviewed +10 batch that uses the M009 runbook gates above.

If the next batch cannot produce real provenance, fresh verification, active lineage, and materialized/preflighted replacements, it should block rather than scan.

## Still blocked

- positive KG import remains blocked;
- production LadybugDB writes remain blocked;
- trusted semantic KG correctness claims remain blocked;
- unattended run-to-100 remains blocked;
- MiniMax as orchestrator/source of truth remains blocked.
