# Human review template

Use one copy of this section per finding. Do not mark a finding resolved from prose, an ADR, or a passing unit test alone when its acceptance criteria require runtime or artifact evidence.

## Review metadata

| Field | Value |
|---|---|
| Review ID | `REV-YYYY-MM-DD-slug` |
| Finding ID | `PIR-NNN` |
| Reviewer |  |
| Date | `YYYY-MM-DD` |
| Tested source revision | git SHA or GSD `sha256:...` where required |
| Environment | OS, toolchain, sidecars, graph mode |

## 1. Finding under review

**Title:**  
**Current status:** `open | in_progress | blocked | resolved | accepted_risk | superseded`  
**Requested status:**  
**Severity / priority:**  

### Claim being tested

> Copy the finding `claim` exactly.

### Risk

> Copy the finding `risk` exactly.

## 2. Post-M397 baseline audit

Complete before interpreting a finding:

| Baseline check | Result | Note |
|---|---|---|
| Canonical StageId / ProductionContext present | PASS/FAIL |  |
| GROBID profile + PDF/HTML manifestations in cache identity | PASS/FAIL |  |
| Exact revision binding or explicit versionless fallback | PASS/FAIL |  |
| Same-key extraction payload without grounding rejected | PASS/FAIL |  |
| D127 remains false | PASS/FAIL |  |
| No 200-paper rebuild or corpus seal claimed | PASS/FAIL |  |

These checks establish the assessment baseline; they do not by themselves resolve open findings.

## 3. Evidence inspected

| Evidence path or execution ID | What was inspected | Source-bound? | Result |
|---|---|---:|---|
|  |  | yes/no | PASS/FAIL/INCONCLUSIVE |

Rules:

- Evidence must exist and be readable.
- A current source revision must be recorded.
- Sidecars and LLM outputs are candidate evidence, not graph truth.
- A cache hit is not evidence that graph materialization occurred.
- Logical counters are not replay proof; use physical store outcomes.
- Cross-process graph claims require a real remote contract; otherwise use same-process snapshot execution.

## 4. Acceptance criteria

| Criterion from JSONL | Result | Evidence | Notes |
|---|---|---|---|
|  | PASS/FAIL/NEEDS-HUMAN |  |  |

## 5. Standards and lock audit

| Standard | Result | Note |
|---|---|---|
| D127 / `import_eligible=false` | PASS/FAIL |  |
| No auto-promote / no deployed inference | PASS/FAIL/N/A |  |
| Samyama sole KG store | PASS/FAIL/N/A |  |
| Revision-owned identity | PASS/FAIL/N/A |  |
| Typed missingness | PASS/FAIL/N/A |  |
| Exact cache key + payload lineage | PASS/FAIL/N/A |  |
| Same-process graph semantics where applicable | PASS/FAIL/N/A |  |
| Onion dependency direction | PASS/FAIL |  |

## 6. Verification commands

```text
# Paste exact commands; do not paste secrets.
```

| Command | Exit | Duration | Durable output |
|---|---:|---:|---|
|  |  |  |  |

Minimum baseline unless not applicable:

```bash
PYTHONPATH=scripts uv run --with pyyaml python -m governor --validate-yaml
PYTHONPATH=scripts uv run --with pyyaml python -m governor --phase active
```

Run per-crate tests during iteration. Run `bash scripts/verify_rust_architecture.sh` only at the slice end.

## 7. Adversarial / negative checks

| Failure injected or invariant challenged | Expected | Actual | Result |
|---|---|---|---|
|  |  |  | PASS/FAIL |

At least one negative check is required for a new Governor rule or a fail-closed contract.

## 8. Decision

Select exactly one:

- [ ] **Resolved** — every acceptance criterion passed with current evidence.
- [ ] **Remain open** — implementation or evidence is incomplete.
- [ ] **Blocked** — external prerequisite is named below.
- [ ] **Accepted risk** — explicit owner, rationale and expiry/revisit trigger are recorded.
- [ ] **Superseded** — replacement finding ID is named.

**Rationale:**  
**Blocker or replacement:**  
**Follow-up finding IDs:**  

## 9. JSONL update proposal

Only update lifecycle fields after evidence review. Preserve `finding_id` and original claim/risk. If remediation changed materially, create a new review revision or a superseding finding.

```json
{
  "finding_id": "PIR-NNN",
  "status": "resolved",
  "verification_evidence": [
    {
      "ref": "artifact-or-execution-id",
      "tested_source_revision": "...",
      "result": "PASS"
    }
  ],
  "reviewer_note": "Why the status change is justified"
}
```
