# GROBID Absorption Evidence Review — Summary (2026-09-10)

Source window: GSD work of 2026-09-08 … 2026-09-10 (M497–M501 closure wave).
Baseline source revision for record evidence: `git:6f328bdfb9b136deecd0ded7917c086e8ad5899b`.
Registry: `doc/review/grobid-absorption-review-2026-09-10.jsonl` (GAR-001…GAR-016).
Status: 15 unresolved / 1 enclosed (GAR-002, superseded administrative correction).
Owner program: `M502-2w7y8k` (GROBID absorption correctness horizon, slices S01…S09).
Locks preserved: `import_eligible=false`, no graph writes, no auto-promote,
no production splice, no PIR-015 splice, no redistribution.

Governor control is read-only registry integrity through
`scripts/governor/checks/grobid_absorption_review.yaml` and
`scripts/governor/adapters/checks/grobid_review_registry.py`:

```bash
PYTHONPATH=scripts python3 -m governor review
```

The command validates JSONL/schema structure, unique ordered GAR IDs,
resolvable dependencies, existing evidence paths, rejection of TODO paths,
explicit `completion_evidence` for enclosed records, and the M502 owner
vocabulary. It never ingests review prose as enforcement and never claims
runtime verification from record existence.

The engine and its producers retain every artifact independently of this
summary. Human-readable narrative is not a source of requirements or scope.

## Findings by owner slice

| ID | Title | Priority | Owner |
|----|-------|----------|-------|
| GAR-001 | Boundary metrics measure boundary-count equality, not boundary positions | P0 | M502-2w7y8k/S02 |
| GAR-002 | Unsupported pg_hba access-control finding is withdrawn pending real evidence | superseded | M502-2w7y8k/S01 |
| GAR-003 | Visual real-runner discards successfully extracted donor primitives | P0 | M502-2w7y8k/S04 |
| GAR-004 | Oracle/model license provenance has unverified items before redistribution | P0 | M502-2w7y8k/S07 |
| GAR-005 | P9 runtime stamp carries a historic conserved verdict while reporting not computed | P1 | M502-2w7y8k/S02 |
| GAR-006 | Synthetic shadow executions share report space with corpus denominators | P1 | M502-2w7y8k/S02 |
| GAR-007 | ReviewReady is unreachable in the maturity derivation despite ladder order | P1 | M502-2w7y8k/S05 |
| GAR-008 | Promotion dry run proves completeness better than evidentiary sufficiency | P1 | M502-2w7y8k/S05 |
| GAR-009 | Freshness identity and revision selection are not bound into evidence receipts | P1 | M502-2w7y8k/S06 |
| GAR-010 | Meta-prompt normatives lack a machine-checkable authority and change-control model | P2 | M502-2w7y8k/S07 |
| GAR-011 | Oracle digests are cross-referenced but not recomputed from canonical source bytes | P1 | M502-2w7y8k/S03 |
| GAR-012 | Production quarantine proves input isolation, not semantic utility or legal admissibility | P1 | M502-2w7y8k/S04 |
| GAR-013 | Citation occurrence, bibliography entry and external work lack explicit link semantics | P1 | M502-2w7y8k/S06 |
| GAR-014 | Cross-automata state invalidation has no interaction map | P1 | M502-2w7y8k/S05 |
| GAR-015 | External holdout remains unexecuted with no acceptance statistics or release evidence | P1 | M502-2w7y8k/S09 |
| GAR-016 | UAT verdict lineage is not machine-checked | P1 | M502-2w7y8k/S01 |

## Exclusions (deliberately rejected fixes)

- **Hash entropy / digest-difference tests for oracle independence.** Repeating
  or random bytes do not prove provenance; identical digests do not disprove
  real measurement. Independence is provenance plus recomputation
  (GAR-011), never a byteshape heuristic.
- **`HumanGateRequired` is not granted authorization.** It means human action is
  awaited. Any `Pending` state must stay distinct from `Granted`; readiness
  must stay distinct from permission (GAR-007, GAR-008).
- **No GPL-violation assertion.** `/root/vendor-source/pdfalto/LICENSE` is
  GPLv2 and GROBID Java is Apache-2.0, but the record asserts only an
  unverified redistribution/provenance gap (GAR-004), not a violation.
- **No mandatory wall-clock freshness oracle.** Freshness is enforced through
  revision ordering, target selection, representation digests and cache keys
  (GAR-009), not through a clock.
- **Nine-slot promotion does not require nine independent executions.**
  Shared evidence sources are permitted when shared failure modes are
  explicitly modelled; completeness checks stay separate from quality
  qualification (GAR-008).

## ADR / document correction matrix

Only point corrections are planned; no lock, authority or routing document
is rewritten into a new source of power.

| Correction | Target documents | Owner slice |
|------------|------------------|-------------|
| Metric/population honesty: count agreement, positional matching, report populations, P9 provenance | ADR-051, ADR-055, ADR-076 | M502-2w7y8k/S02 |
| Oracle recomputation and producer provenance | oracle/shadow contracts and fixtures | M502-2w7y8k/S03 |
| Visual adapter content continuity and missingness | visual shadow contracts and fixtures | M502-2w7y8k/S04 |
| FSM vocabulary: evidence status vs authorization status, completeness vs qualification | FSM dictionaries only, no new authority | M502-2w7y8k/S05 |
| Revision/freshness identity and citation link semantics | ADR-058, ADR-065, only where semantics changes | M502-2w7y8k/S06 |
| Per-component licenses, subprocess-only boundaries, normative prompt contracts | ADR-075/ADR-076, `grobid_compatibility.yaml`, `data/ontology/prompt_tasks.yaml`, prompt files | M502-2w7y8k/S07 |
| Real vertical scenario without stubs and external holdout/release package | `data/parse/acceptance_manifest.yaml` | M502-2w7y8k/S08–S09 |

## Trust model

- The JSONL registry is a control dataset, not proof.
- `governor.coverage=gap` means the named `proposed_rule_id` is a backlog
  identifier until a deterministic scanner/contract with positive and
  negative fixtures exists.
- Enclosed findings carry `completion_evidence` with mode, ref, pass result,
  existing artifact paths, observation date and source revision.
- Historical PIR-001…PIR-022 records remain untouched; this GAR register is
  the active corrective register behind the canonical M502 program.
