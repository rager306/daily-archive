# M170 Acceptance Contract

## Purpose

M170 combines three requested next steps in one milestone:

1. bounded architecture backlog remediation;
2. longer UniversalKBQueue soak;
3. same-key cache write coordination policy or implementation.

This contract prevents the milestone from turning into broad architecture churn.

## Required pass conditions

### Global ratchets

M170 must preserve:

```text
allowlisted_dynamic_script_import=0
allowlisted_legacy_mixed=0
test_architecture_violations=0
onion_violation_count=0
onion_allowed_violation_count=0
write_path_unknown=0
```

### Architecture backlog track

Pass requires:

- `data/architecture-assessment/m170-architecture-backlog-inventory.md` remains the source of scope;
- the four `shared-state` write records are reviewed and each receives an ownership or risk disposition;
- any architecture code change is tied to one listed target;
- no full repository strict architecture claim is made.

Allowed outcomes:

1. **Code remediation** for a concrete unsafe shared-state record.
2. **Policy-only closure** when records are safe by ownership or run mode.
3. **Explicit deferral** when a record needs a separate milestone.

### Cache coordination track

Pass requires:

- same-key stable CLI/PDF cache behavior is reviewed;
- a policy decision compares atomic-only, lock-file, and compare-and-swap style approaches;
- if code is needed, it is bounded to CLI per-paper JSON and PDF cache writes;
- if code is not needed, the no-code decision must state residual risk and activation trigger.

Allowed outcomes:

1. **Atomic-only accepted for current scope** with documented trigger for future lock/CAS.
2. **Minimal lock-file implementation** if concurrent same-key writers are likely.
3. **Minimal compare-and-swap style implementation** only if stale overwrite detection is necessary and testable.

### Queue soak track

Pass requires:

- longer soak parameters are defined before runtime;
- soak runs separate processes with separate queue connections;
- soak result includes jobs, processes, rounds, timeout, worker diagnostics, completion counts, and uniqueness checks;
- every job completes exactly once;
- no worker process hangs beyond the configured timeout.

Allowed outcomes:

1. **Reusable harness plus runtime proof**.
2. **Existing test reuse with stronger parameters** if no new harness is needed.
3. **Documented blocker** only if process-level runtime exposes a real queue issue.

## Non-goals

M170 must not:

- claim full repository strict architecture compliance;
- migrate all historical scripts;
- add global locking for every artifact write;
- clean unrelated archive shim ruff debt;
- weaken strict onion or test architecture guardrails;
- hide `shared-state` records through broad scanner categories;
- push to a remote repository.

## Required verification before closeout

Run and record:

```text
uv run python scripts/verify_test_architecture.py --json
uv run python scripts/verify_onion_layering.py --json
uv run python scripts/inventory_write_paths.py --json data/architecture-assessment/m170-write-path-inventory-final.json --markdown data/architecture-assessment/m170-write-path-inventory-final.md
uv run pytest <focused M170 tests> -q
uv run ruff check <changed Python files>
uv run pyrefly check
uv run pre-commit run --all-files
gitnexus_detect_changes scope=unstaged or compare
```

The exact focused pytest target is selected after implementation slices, but it must include any changed queue, CLI, PDF downloader, shared-state, and guardrail tests.

## Residual risk policy

Residual risks are acceptable only when:

- they are named in the closeout artifact;
- they have a trigger for future work;
- they do not contradict a pass condition above.

Known likely residuals:

1. bounded queue soak is not a production-duration stress run;
2. atomic-only cache coordination may remain sufficient until same-key concurrent writers become real;
3. full system async and multithread readiness remains broader than M170.

## Closeout artifacts required

- `data/architecture-assessment/m170-shared-state-review.md`
- `data/architecture-assessment/m170-cache-coordination-policy.md`
- `data/architecture-assessment/m170-queue-soak-contract.md`
- `data/architecture-assessment/m170-queue-soak-result.md`
- `data/architecture-assessment/m170-integrated-verification.md`
- `data/architecture-assessment/m170-hardening-closeout.md`
