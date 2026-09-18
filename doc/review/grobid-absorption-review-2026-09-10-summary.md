# GROBID Absorption Evidence Review — Summary (2026-09-10)

Source window: GSD work of 2026-09-08 … 2026-09-10 (M497–M501 closure wave).
Baseline source revision for record evidence: `git:6f328bdfb9b136deecd0ded7917c086e8ad5899b`.
Registry: `doc/review/grobid-absorption-review-2026-09-10.jsonl` (GAR-001…GAR-016).
Status: 3 unresolved / 13 enclosed (4 P0 findings resolved by M508-g9b5gi/S05: GAR-001, GAR-003, GAR-004, GAR-011; the eight P1 findings GAR-005..GAR-009 and GAR-012..GAR-014 resolved by M509-yzblqx S01..S05; GAR-002 superseded administrative correction from 2026-09-10).
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
| GAR-005 | P9 runtime stamp carries a historic conserved verdict while reporting not computed | P1 | M509-yzblqx/S01 (resolved) |
| GAR-006 | Synthetic shadow executions share report space with corpus denominators | P1 | M509-yzblqx/S02 (resolved) |
| GAR-007 | ReviewReady is unreachable in the maturity derivation despite ladder order | P1 | M509-yzblqx/S03 (resolved) |
| GAR-008 | Promotion dry run proves completeness better than evidentiary sufficiency | P1 | M509-yzblqx/S03 (resolved) |
| GAR-009 | Freshness identity and revision selection are not bound into evidence receipts | P1 | M509-yzblqx/S01 (resolved) |
| GAR-010 | Meta-prompt normatives lack a machine-checkable authority and change-control model | P2 | M502-2w7y8k/S07 |
| GAR-011 | Oracle digests are cross-referenced but not recomputed from canonical source bytes | P1 | M502-2w7y8k/S03 |
| GAR-012 | Production quarantine proves input isolation, not semantic utility or legal admissibility | P1 | M509-yzblqx/S04 (resolved) |
| GAR-013 | Citation occurrence, bibliography entry and external work lack explicit link semantics | P1 | M509-yzblqx/S05 (resolved) |
| GAR-014 | Cross-automata state invalidation has no interaction map | P1 | M509-yzblqx/S04 (resolved) |
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

## Closure (M508-g9b5gi/S05)

Four P0 findings were closed by wave `M508-g9b5gi/S05` on 2026-09-17 by
acknowledging controls that slices S01..S04 had already delivered. The closure
is machine-checkable rather than textual: each record now carries
`completion_evidence` with `mode`, `ref`, `result: pass`, a non-empty list of
existing committed `artifacts`, `observed_at` and `source_revision`, and the
registry schema forbids additional keys (`additionalProperties: false`). When
this register was created on 2026-09-10 the number of unresolved records was
fifteen and one record was superseded; those four closures plus that earlier
supersession are the 2026-09-17 state recorded in §10 of
`doc/adr/ADR-076-grobid-cascade-three-automata-graph-freeze.md`.
Wave `M509-yzblqx` then closed the eight P1 findings (GAR-005..GAR-009 and
GAR-012..GAR-014) through slices S01..S05, each record now carrying six-key
`completion_evidence` at source revision
`git:2871d87c9c9e4d46f49d76edeb2e6c33f17ba970`; together with the four P0
closures and the GAR-002 supersession this leaves the current state at
3 unresolved / 13 enclosed.

| Finding | Status | Governor rule | Executable check |
|---|---|---|---|
| GAR-001 | resolved | `boundary_positional_metrics` | `cargo test -p da-cli --features pdf-layout --example document_segmentation_shadow boundary_position_negative` |
| GAR-003 | resolved | `visual_adapter_preserves_donor_content` | `cargo test -p da-cli --features pdf-layout --test visual_adapter_contract` |
| GAR-004 | resolved | `component_license_provenance` | `cargo test -p da-parse --test component_license_provenance_data_contract` |
| GAR-011 | resolved | `oracle_digest_recomputation` | `cargo test -p da-parse --test oracle_digest_recomputation` |

Caveats that closure does not erase:

- **GAR-003** — the donor sidecar (`VisualObservation`) supplies primitives and
  provenance; the seven receipt families (captions, callouts, cells, labels)
  are not supplied by it and remain an explicit residual (D394). The wording
  must say "donor sidecar" so the closure is not read as R094.
- **GAR-004** and **GAR-011** — `governor.coverage` stays `partial` on purpose:
  `partial` describes the coverage of the governor rule, not the completeness of
  the closure. The closed verified-license dictionary and the
  verified-versus-blocked surface check live only in the Rust reader (MEM1505);
  the sha256 mathematics is recomputed by the independent Rust contract
  (D406/MEM1527); and the GAR-011 audit is bounded to the segmentation surface
  (D404).
- No lock was weakened: `deployed=false`, `import_eligible=false`, `auto_promote`
  remains forbidden, there is no production splice, and no S01..S04 control was
  modified.

## Closure (M509-yzblqx/S07)

Eight P1 findings were closed by wave `M509-yzblqx` (slices S01..S05) on
2026-09-18. The closure is machine-checkable rather than textual: each record
now carries the same six-key `completion_evidence` contract used by the
2026-09-17 P0 closure (`mode`, `ref`, `result: pass`, a non-empty list of
existing committed `artifacts`, `observed_at`, and `source_revision`), and the
registry schema keeps `additionalProperties: false`. All eight records cite
source revision `git:2871d87c9c9e4d46f49d76edeb2e6c33f17ba970`.

| Finding | Owner slice | Governor rule | Executable check |
|---|---|---|---|
| GAR-005 | M509-yzblqx/S01 | `conservation_verdict_provenance` (existing) | `cargo test -p da-cli --test conservation_provenance` |
| GAR-006 | M509-yzblqx/S02 | `executed_population_binding` (existing) | `cargo test -p da-parse --test data_contracts` |
| GAR-007 | M509-yzblqx/S03 | `maturity_authorization_separation` (gap: proposal only, not an active rule) | `cargo test -p da-application --test maturity_transition` |
| GAR-008 | M509-yzblqx/S03 | `promotion_completeness_vs_qualification` (partial; `promotion_evidence_contract` is the active rule) | `cargo test -p da-application --test promotion_qualification` |
| GAR-009 | M509-yzblqx/S01 | `freshness_identity` (gap: proposal only, not an active rule) | `cargo test -p da-application --test freshness_identity` |
| GAR-012 | M509-yzblqx/S04 | `quarantine_semantic_boundary` (existing) | `PYTHONPATH=scripts python3 -m pytest scripts/tests/test_governor.py -k "quarantine_semantic_boundary" -q` |
| GAR-013 | M509-yzblqx/S05 | `citation_link_contract` (gap: proposal only, not an active rule) | `cargo test -p da-ports --test citation_link_contract` |
| GAR-014 | M509-yzblqx/S04 | `fsm_interaction_map` (existing) | `PYTHONPATH=scripts python3 -m pytest scripts/tests/test_governor.py -k "fsm_interaction_map" -q` |

`governor review` reports the refreshed register state as a machine-readable
status line:

```bash
PYTHONPATH=scripts python3 -m governor review --registry scripts/governor/checks/grobid_absorption_review.yaml
# review registry: records=16 unresolved=3 enclosed=13 unassigned_unresolved=0
```

No lock was weakened by this closure: `deployed=false`, `import_eligible=false`,
`auto_promote` remains forbidden, and no production splice was added. The three
remaining open records (GAR-010, GAR-015, GAR-016) stay named in
`manifest.yaml` under `long_horizon.next_wave`, and the 2026-09-10 source-window
framing above is unchanged.
