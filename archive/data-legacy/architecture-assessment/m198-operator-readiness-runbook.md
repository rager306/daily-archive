# M198 Operator Readiness Runbook

## Purpose

This runbook explains how to run and interpret the M198 reactive readiness precondition checks. M198 is a no-write readiness milestone: it compares readiness evidence and packages validation results, but it does not authorize production graph import, schema migration, queue dependency semantic changes, smoke semantic changes, rehearsal semantic changes, or import eligibility promotion.

## Required Inputs

- Metadata-only readiness evidence for S08 indexing.
- S12 impact gate contract: `data/architecture-assessment/m198-gitnexus-impact-gates.json`.
- A temp work directory for rehearsal outputs.

## Command Sequence

Run commands from `/root/daily-archive` with `uv run`.

### 1. Rehearse readiness

```bash
uv run python scripts/run_m198_readiness_rehearsal.py \
  --workdir tmp/m198-readiness-work \
  --summary tmp/m198-readiness-rehearsal.json \
  --markdown tmp/m198-readiness-rehearsal.md \
  --mode ready
```

Expected contract: `m198.readiness_rehearsal.v1`.

Expected exit codes:

- `0`: rehearsal verdict is ready.
- `2`: rehearsal verdict is blocked; inspect `blockers`.

### 2. Audit smoke parity

```bash
uv run python scripts/run_m198_smoke_parity_audit.py \
  --rehearsal tmp/m198-readiness-rehearsal.json \
  --audit tmp/m198-smoke-parity.json \
  --markdown tmp/m198-smoke-parity.md
```

Expected contract: `m198.smoke_parity_audit.v1`.

Expected exit codes:

- `0`: smoke parity status is pass.
- `2`: smoke parity status is fail; inspect `failed_checks` and `blockers`.

### 3. Audit disabled backend safety

```bash
uv run python scripts/run_m198_disabled_backend_safety.py \
  --audit tmp/m198-disabled-backend-safety.json \
  --markdown tmp/m198-disabled-backend-safety.md
```

Expected contract: `m198.disabled_backend_safety.v1`.

Expected exit codes:

- `0`: disabled backend safety status is pass.
- `2`: disabled backend safety status is fail; inspect `failed_checks` and `blockers`.

### 4. Build validation package

```bash
uv run python scripts/run_m198_validation_package.py \
  --impact-gates data/architecture-assessment/m198-gitnexus-impact-gates.json \
  --rehearsal tmp/m198-readiness-rehearsal.json \
  --smoke-parity tmp/m198-smoke-parity.json \
  --disabled-backend tmp/m198-disabled-backend-safety.json \
  --package tmp/m198-validation-package.json \
  --markdown tmp/m198-validation-package.md
```

Expected contract: `m198.validation_package.v1`.

Expected exit codes:

- `0`: validation package status is pass.
- `2`: validation package status is fail; inspect aggregate `blockers` first.

## Interpreting Results

- Start with the validation package `status`, `ready`, and `blockers` fields.
- If the package fails because an input failed, inspect that input artifact next.
- If `metadata_only` or `payload_policy_confirmed` is false, stop and fix the producer of that artifact before rerunning.
- If no-write boundary confirmations are not false, stop and treat readiness as blocked.
- Warnings do not authorize promotion; they are review items for the next milestone.

## GitNexus Discipline

- After committing new symbols, refresh the index from the repo root with `gitnexus analyze`.
- Before future function, class, or method edits, run exact GitNexus impact with `repo=daily-archive`.
- Before commit, run scoped detect_changes with `repo=daily-archive`.
- If impact is HIGH or CRITICAL, warn before editing and run the relevant compatibility tests.

## Required Verification Set

For final M198 verification, run:

```bash
uv run pytest \
  tests/test_m198_validation_package.py \
  tests/test_m198_disabled_backend_safety.py \
  tests/test_projection_backend_seams.py \
  tests/test_m198_smoke_parity_audit.py \
  tests/test_m198_readiness_rehearsal.py \
  tests/test_m198_gitnexus_impact_gates.py \
  tests/test_m198_no_write_governance.py \
  tests/test_m198_readiness_report.py \
  tests/test_m198_operator_diagnostics.py \
  tests/test_m198_evidence_index.py \
  tests/test_m198_drift_classifier.py \
  tests/test_m198_readiness_evidence_contract.py \
  tests/test_m197_governance_ratchets.py \
  tests/test_m196_governance_ratchets.py \
  tests/test_m195_governance_ratchets.py \
  -q
uv run ruff check \
  scripts/run_m198_validation_package.py \
  tests/test_m198_validation_package.py \
  tests/test_m198_operator_runbook.py
uv run pyrefly check
```

## Non Goals That Remain Blocked

- Production graph import.
- Schema migration.
- Queue dependency semantic changes.
- Smoke runtime semantic changes.
- Rehearsal runtime semantic changes.
- Retired graph readiness shim restoration.
- Import eligibility promotion.
- Raw payload, embedding, vector, secret, or credential exposure.

## Handoff To S18

S18 should use this runbook and the validation package to complete final validation and milestone closeout. The closeout should report requirement outcomes for R076, R077, and R078 with evidence from S13-S17 artifacts.
