# M170 Architecture Backlog Inventory

## Verdict

**M170 architecture backlog is bounded and feasible.**

The broad M165 finding was: strict import architecture is compliant, but full repository strictness was still partial because tests, write safety, and concurrency readiness were not fully proven. M166-M169 closed most of that backlog. M170 should not reopen broad architecture refactors; it should close the remaining concrete follow-ups.

## Source evidence

Extraction evidence: `gsd_exec[370ce5ce-8b10-4267-83ca-c9b6001891c4]`.

Baseline evidence:

- `data/architecture-assessment/m170-baseline.md`
- `data/architecture-assessment/m170-feasibility.md`
- `data/architecture-assessment/m170-write-path-inventory.json`
- `data/architecture-assessment/m170-write-path-inventory.md`

## Candidate backlog classification

| Candidate | Source | Current state | M170 classification | Rationale |
|---|---|---|---|---|
| Dynamic and legacy test allowlists | M165, M167, M168 | Closed in M169: dynamic=0, legacy=0 | Already closed | Preserve ratchet, do not reopen. |
| Unknown write-path records | M167, M168 | Closed in M169: unknown=0 | Already closed | Preserve inventory, do not hide shared-state records. |
| Canonical catalog atomic writes | M167, M168 | Closed in M168 | Already closed | `catalog_ingest.py` article/index writes were hardened with atomic sibling-temp replace. |
| Remaining same-key cache write coordination | M169 residual risk | Atomic replacement exists for CLI/PDF stable outputs, but no lock/CAS policy | In scope | User explicitly selected next step 3; S04-S08 decide whether lock/CAS is needed or no-code policy is enough. |
| Longer UniversalKBQueue soak | M168 and M169 residual risk | M169 has bounded 16-job/4-process pytest proof | In scope | User explicitly selected next step 2; S09-S11 add longer configurable soak proof. |
| Shared-state write-path records | M169 and M170 inventory | `shared-state=4` remains visible | In scope for review | Review only; code changes only if a concrete unsafe record is found. |
| Full async or multithread readiness claim | M165 | Still broader than current evidence | Deferred | Too broad for one milestone; M170 can add queue soak evidence but must not claim full system readiness. |
| Runtime purity beyond import checks | M165 | Not systematically proven | Deferred | Needs separate design and instrumentation milestone; no clear small remediation target yet. |
| Global script wrapper migration | M165 | Historical scripts remain script-only and not imported by package | Deferred | No current guard violation; migrating all scripts would be broad churn. |
| Locking every shared artifact write | M165 write policy note | Not implemented globally | Deferred | Locking everything would be speculative; M170 will assess only same-key stable cache writes and four shared-state records. |

## Shared-state records for S04

From `data/architecture-assessment/m170-write-path-inventory.md`:

| Path | Line | Target | Initial review question |
|---|---:|---|---|
| `src/research_graph/application/validation/batch_state.py` | 252 | `output_path` | Is this caller-owned validation output or true shared state needing atomic or lock policy? |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py` | 540 | `summary_path` | Is summary write single-writer by catalog adapter invocation or shared across processes? |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py` | 935 | `report_path` | Is report path run-scoped, caller-owned, or a stable shared report? |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 183 | `index_path` | Is this a stable shared baseline index or a caller-owned measurement output? |

## Selected M170 architecture remediation targets

### Target A: Shared-state ownership review and ratchet

Slices: S04, S08, S14.

Closure condition:

- all four shared-state records have explicit ownership classification;
- inventory remains `unknown=0`;
- no broad scanner weakening occurs;
- if a record is found unsafe, a targeted remediation slice is added or the relevant existing slice is used.

### Target B: Same-key cache coordination policy

Slices: S05-S08.

Closure condition:

- policy decision is explicit;
- if code is warranted, it is bounded to same-key stable CLI/PDF cache writes;
- if no code is warranted, residual risk and activation trigger are documented;
- focused tests or no-code policy artifact verifies closure.

### Target C: Longer queue soak readiness

Slices: S09-S11.

Closure condition:

- configurable process-level soak harness exists or a simpler existing harness is reused;
- runtime proof completes under bounded timeout;
- result captures worker diagnostics and every job completes exactly once.

## Explicit deferrals

1. **Full repository strict architecture claim**: deferred until runtime purity, concurrency, cancellation, and all shared-state policies are proven beyond import and focused soak checks.
2. **Global historical script migration**: deferred because current package import boundaries and test architecture ratchets are green.
3. **Global lock policy for every artifact write**: deferred until a real multi-writer consumer or activation path requires it.
4. **Unrelated archive shim ruff cleanup**: deferred; known unrelated debt should not block this milestone.

## Acceptance implications for S03

S03 should preserve these pass conditions:

- `allowlisted_dynamic_script_import=0`.
- `allowlisted_legacy_mixed=0`.
- strict onion `allowed_violation_count=0`.
- write-path `unknown=0`.
- shared-state records remain visible unless each has a concrete safer classification.
- M170 may improve readiness evidence, but must not claim full system strict async or multithread readiness.
