# Next milestone spec — DRAFT for human review

**Status:** DRAFT. Not created. Awaiting owner review before `gsd new-milestone`.
**Drafted:** 2026-10-03, after M514-1lss2s was parked with `needs-attention`.
**Scope source:** recorded evidence only. Nothing here is invented; every item cites a
finding that a validator already produced.

---

## 0. One-screen summary (for the approval decision)

**What it fixes.** `514-VALIDATION.md` blocked M514 because the three frozen thresholds
resolve `not_comparable` (`metric_absent`) — no frozen metric is measured at all — plus a
missing same-n Java resource reference and partial R134/R140/R142 coverage. Rule C2:
"without frozen thresholds there is no acceptance."

**What the spikes found (S01–S05).** Every validator complaint turned out to be a
**missing explicit linkage** against a mechanism or piece of evidence that already exists,
is committed, and is already independently judged. This is a **wiring milestone**, not a
build-from-scratch one.

**Six slices:** S01 composition-scanner consumption; S02 route→class-parity wiring;
S03 same-n Java resource pairing; S04 R134 recompute/provenance linkage;
S05 R140/R142 evidence linkage; S06 re-resolve thresholds, re-issue the package, run the
full gate.

**Success means measurable and honestly reported, not "thresholds passed".** If the native
plane measures below a frozen reference, that is the result and it is reported as-is.

**Out of scope:** promotion, deploy, import, graph writes (all five locks stay closed);
`data/parse/acceptance_manifest.yaml` and the M513 release package stay read-only; the S09
human release record is written by a human, never an agent.

**Decision needed:** approve as-is, or name the changes. Nothing is created until then.

---

## 1. Why this milestone exists

`514-VALIDATION.md` closed M514 as `needs-attention`. Its own rationale reads:

> Two of three independent reviewers returned NEEDS-ATTENTION and one returned PASS; no
> reviewer observed a FAIL, and all nine slice assessments are PASS. ... the attention
> items are partial requirement coverage (R134/R140/R142), a DB attribution gap for
> S08/S09, and inconclusive acceptance evidence (frozen thresholds all
> not_comparable/cross-series, container Java CPU not_observed, and no genuine human
> release authorization for S09). These are missing/inconclusive evidence and
> explicitly-scoped unmet preconditions, not observed failures.

Two of those items were closed after validation (see §6). The rest are **missing
measurement machinery**, not defects. `acceptance criterion C2` states the rule:

> Without frozen thresholds there is no acceptance.

So M514 cannot reach `pass` until the frozen metrics are actually measurable. M514's own
T01 deliberately refused to build that machinery and named it as a gate instead:

> NOT required: building a production route→class-parity bridge, constructing route
> oracles, or porting the ~2.5k-line measurement engine — the absence of each must be
> NAMED as a gate, not built here.

This milestone is the named gate, built.

---

## 2. Measured state today (committed bytes)

`artifacts/parse_compat/native_corpus_acceptance.json`: populations
`assigned=610, measurable=531, refused=0, excluded=79`.

Only three metrics are produced, all coverage/unit counts with no quality ratio:

| metric_id | denominator | value |
|---|---|---|
| `native_acceptance.pdf_verification_coverage` | 610 | none |
| `native_acceptance.native_plane_coverage` | 531 | none |
| `native_acceptance.native_plane_units` | 531 | none |

**No frozen-threshold metric is measured at all.**

---

## 3. The gap, exactly

`grobid_retirement_acceptance_package.json#evaluation` — all three thresholds resolve
`not_comparable`, reason `metric_absent`, candidate value `null`:

| threshold_id | metric_id | family_id | frozen reference |
|---|---|---|---|
| `preservation_micro_occurrence` | `preservation.occurrence_conservation` | `parse.component.native_fulltext_composition_shadow` | 0.9408321925120258 |
| `extraction_micro_occurrence` | `extraction.native_capability_recall` | `stage.reference_segmentation` | 0.945771 |
| `extraction_weakest_family` | `extraction.native_capability_recall` | `stage.header_composition` | 0.4316800904337996 |

And the 15 typed gaps, grouped by reason (from `native_corpus_acceptance.json#gaps`):

| reason | count | families |
|---|---|---|
| `producing_mechanism_absent:s01_composition_scanner_consumes_composed_emission` | 1 | `parse.component.native_fulltext_composition_shadow` |
| `class_parity_axis_absent:not_a_native_route_product` | 2 | `stage.reference_segmentation`, `stage.header_composition` |
| `rest_transport_surface` | 6 | `rest.isalive/health/version/processFulltextDocument/processHeaderDocument/processReferences` |
| `dialect_flag_surface` | 1 | `dialect.teiCoordinates` |
| `pdf_input_stage_not_in_tei_composition` | 1 | `stage.pdfalto_layout` |
| `lane_not_admitted_in_measurement` | 1 | `stage.document_segmentation` |
| `visual_lane_not_admitted_in_measurement` | 1 | `stage.figure_table_formula` |
| `java_side_profile_surface` | 1 | `stage.bibliographic_consolidation` |
| `not_in_measured_runner_path` | 1 | `stage.tei_lift` |

The three reasons that block C2 are the first two rows. The other rows are scope the
product does not implement (transport surface, dialect flags, pdfalto stage, visual lane)
and are expected to stay typed absences.

---

## 4. Proposed scope (slices)

Ordered; each slice is bounded and has a named proof.

- **S01 — Composition scanner consumes the composed emission.**
  Retires `producing_mechanism_absent:s01_composition_scanner_consumes_composed_emission`
  so `preservation.occurrence_conservation` is measurable on
  `parse.component.native_fulltext_composition_shadow`. Proof: the metric carries a
  numerator and denominator on the reference series, and the evaluator returns
  `met | unmet` instead of `not_comparable`.

- **S02 — Route→class-parity bridge and route oracles.**
  Retires `class_parity_axis_absent:not_a_native_route_product` (×2) so
  `extraction.native_capability_recall` is measurable on `stage.reference_segmentation`
  and `stage.header_composition`, micro and weakest-stratum. Proof: same as S01, for both
  extraction thresholds.

  **Spike result (2026-10-03) — what the bridge actually is.** The frozen references are
  not opaque numbers; `absorption_progress_reference_pins.json#values` names their exact
  derivation, so S02 is a bounded implementation, not an open-ended port:

  | threshold | fraction | derivation (verbatim) |
  |---|---|---|
  | `preservation_micro_occurrence` | 74910/79621 = 0.94083 | `evaluated_population.aggregate.admitted / source_population` |
  | `extraction_micro_occurrence` | 945771/1e6 = 0.94577 | `evaluated_population.class_parity.aggregate.native_capability_recall_micros / 1e6` |
  | `extraction_weakest_family` | 3055/7077 = 0.43168 | `minimum conserved/source_population across class_profiles classes` |

  So S02 needs three named things over the native route product: (a) a **class-profile
  projection** carrying per-class `conserved`/`source_population`, (b) **micros
  aggregation** to 1e6, (c) the **weakest-class minimum** across class profiles. S01
  separately needs an admitted-vs-source count on the composed emission. Nothing here
  requires reproducing Java internals — it requires projecting the same axes onto the
  native output that the frozen references already aggregate.

  **Spike result — the producers already exist; the mismatch is a producing path.**
  `crates/da-cli/examples/native_corpus_acceptance.rs` (T01) states the mismatch itself,
  in its own module docs:

  > Where a frozen metric is natively observable *through this runner's own producing
  > path*, it is measured with a non-empty denominator. Where the producing mechanism is
  > absent, the runner emits a typed gap cell (`not_observable` with a machine-readable
  > reason): the class-parity axis behind the two extraction thresholds is not a native
  > route product, and the `native_fulltext_composition_shadow` scanner behind the
  > preservation threshold consumes the composed emission, not the native route result.

  The reference producer already exists and is named in the pins' source artifact:
  `external_holdout_release_probe.json#evaluated_population.class_parity.producer` =
  "da-cli compact non-validating scan over the composed emission", with an independent
  judge at `crates/da-parse/tests/external_holdout_probe_semantics.rs` (roxmltree,
  170 KB, committed). `class_parity` / `native_capability_recall` already appear in
  `crates/da-cli/examples/external_holdout_release_probe.rs`.

  So S01/S02 are **wiring and projection onto mechanisms that already exist and are
  already independently judged**, not net-new engine construction. S01/S02 must either
  (i) make the acceptance runner consume the composed emission on the same path the
  probe already scans, or (ii) make the native route product expose the class-parity
  axis. Re-estimate from that decision, not from a line count.

  **Spike result — the inventory is concrete.**

  | piece | path | size |
  |---|---|---|
  | reference producer (the "measurement engine") | `crates/da-cli/examples/external_holdout_release_probe.rs` | 4,762 lines, committed |
  | its contract test | `crates/da-parse/tests/external_holdout_probe_contract.rs` | 1,591 lines |
  | independent semantic judge (roxmltree) | `crates/da-parse/tests/external_holdout_probe_semantics.rs` | 170 KB |
  | contracts already exercising `class_parity` | `external_holdout_release_probe_test.rs`, `data_contracts.rs`, `external_holdout_probe_contract.rs`, `external_holdout_probe_semantics.rs`, `grobid_retirement_package_doc_contract.rs`, `parity_report_contract.rs` | 6 files |
  | judge material for the preservation axis | the above plus `native_fulltext_corpus_data_contract.rs`, `native_fulltext_corpus_semantics.rs`, `native_fulltext_corpus_mutations.rs`, `native_citation_oracle.rs`, `fixtures/holdout/frozen_tables.json` | 7 sources |

  The "~2.5k-line measurement engine" M514 deferred is therefore the probe example that
  already exists at 4,762 lines. S01/S02 do not rebuild it; they make the acceptance
  runner consume the axes it already produces — while keeping the acceptance run
  hermetic and bounded, and never letting the probe's `local_proxy_only` claim scope
  leak into the acceptance claim.

  **Spike result — implementation sketch.** In `native_corpus_acceptance.rs` the two gaps
  are not discovered, they are *hardcoded* in `build_gap_ledger(expectation)`:

  ```rust
  let reason = match threshold.family_id.as_str() {
      "parse.component.native_fulltext_composition_shadow" =>
          "producing_mechanism_absent:s01_composition_scanner_consumes_composed_emission",
      _ => "class_parity_axis_absent:not_a_native_route_product",
  };
  ```

  So the shape of both slices is the same and it is bounded:
  1. extend the per-document fold in `run(&Args)` (and the incremental resume cache) so
     each document also carries the composed-emission scan numbers — the axes the probe
     already computes;
  2. replace the unconditional gap construction in `build_gap_ledger` with a
     measure-or-gap decision: emit `met | unmet` with a non-empty denominator when the
     axis covers the document, and keep the typed gap cell only where it genuinely
     does not;
  3. keep the `--self-check` mutation matrix green, and add the two mutations that now
     matter: a scan that shrinks the denominator, and a class-parity projection that
     drops the weakest class.

  That is a wiring change against an existing, independently judged producer — not a new
  engine. S02 additionally decides option (i) vs (ii) above; option (i) is smaller and
  keeps the acceptance claim on the composed emission the pins were derived from.

- **S03 — Same-n Java resource reference.**
  Retires `resource.container_java_cpu = not_observed{reference_row_not_same_n}` by
  producing a same-n Java CPU reference for the paired resource envelope. Proof: the
  operational axis reports a measured pair instead of a typed absence, and
  `pairing=same_documents_only` / `pooled=forbidden` stay enforced.

  **Spike result (2026-10-03) — the reference partially exists, and the obstacle is
  pairing discipline, not measurement.** `external_holdout_release_probe.json#latency_accounting`:

  ```json
  {"documents": 531,
   "latency_source": "crates/da-parse/tests/fixtures/holdout/provenance.yaml#documents[].latency_ms",
   "measurement_source": "recorded_wall_clock_per_document",
   "min_latency_ms": 1469, "max_latency_ms": 64611,
   "pooled_latency_claim": "forbidden", "wall_clock_at_run_time": false}
  ```

  So a per-document Java latency reference already exists for 531 documents — the very
  `provenance.yaml` carrier M514/S08/T03 first tripped over, because it covers a
  different document set under a different pin. S03 is therefore the same pairing
  problem S08/T04 already solved for a different envelope: pair only where the pinned
  same-n row covers **exactly** the same documents and the same metric, with
  `mismatch=hard_failure`, `pooled=forbidden`, `absence_policy=typed_not_observed`.

  Cost does **not** need measuring: `cost_accounting.status = "bounded_not_measured"`,
  with `unknown_is_not_zero: true` and bounds referenced from
  `data/parse/acceptance_manifest.yaml#external_holdout_contract.cost_bounds`. S03 must
  keep it that way rather than manufacturing a cost number.

  Also carried verbatim from the probe's `named_limitations` and to be preserved by the
  acceptance package: 79 documents are excluded **by name**, never silently dropped;
  531 documents is a count, not a statistical corpus; and the probe **does not claim
  independence from the production route** — the reference was produced by the declared
  live Java release and the admitted band is a local proxy.

- **S04 — Independent recompute and provenance evidence (R134).**
  Adds the explicit reviewer-recomputability-from-readable-source-bytes check and the
  same-generator/different-label provenance check the R134 reviewer found absent. Proof:
  both checks fail closed on an injected mutation and pass on the committed carriers.

  **Spike result (2026-10-03) — both checks already exist and are already tested.** The
  reviewer's finding is about the M514 carriers, not about the codebase:

  | requirement | existing mechanism |
  |---|---|
  | same-generator / different-label provenance | `crates/da-application/tests/promotion_qualification.rs::shared_producer_digest_is_unqualified`, plus the `PromotionGateBlockReason::ProducerNotIndependent` block reason in `crates/da-application/src/native_family_promotion_gate.rs` (lines 76, 328, 830) |
  | producer-independence thresholds | `promotion_qualification.rs::weak_producer_independence_is_unqualified_and_thresholds_are_explicit` |
  | recompute from readable bytes | recompute checks already present in `crates/da-parse/tests/`: `native_header_same_n.rs`, `native_bibliography_same_n.rs`, `native_fulltext_same_n.rs`, `native_acceptance_evaluator_contract.rs`, `batch_contract.rs`, `external_holdout_probe_semantics.rs` |

  So S04 is mostly **pointing the existing discipline at the M514 carriers and recording
  the linkage**, plus the one genuinely missing piece: an explicit
  reviewer-recomputability statement over the acceptance package's own bytes. It is not
  new mechanism construction. Do not restate or duplicate `promotion_qualification`'s
  assertions — reuse them.

- **S05 — R140 / R142 evidence closure.**
  Makes the conservation/contamination measurement and reward versioning R142 demands
  explicit, and re-states R140's delivered-vs-typed-absence boundary with measured
  numbers. Proof: the named measurements are present and pinned.

  **Spike result (2026-10-03) — the evidence exists; the reviewer asked for an explicit
  pointer.** `artifacts/parse_learning/candidate_cycle.json` already carries
  `candidate_id`, `weights_digest`, `split`, `prompt_lineage_digest`,
  **`reward_definition_sha256`** (this is the reward versioning R142 names), `gain_rule`,
  `baseline_n`, `candidate_n`, `baseline_metric_one/two`, `candidate_metric_one/two`,
  `metric_one_delta`, `metric_two_delta`, **`validation_gap`**, `quality_qualification`,
  `critic_failures`, and conservation/contamination entries.
  `artifacts/parse_learning/learning_corpus.json` carries `reward_definition`,
  `gain_rule`, `train`, `validation`, **`corpus_digest`**, `split_rule`,
  `document_identity_field`, `prompt_identity_role`.

  So R142's "does not explicitly show" is a **linkage statement, not a missing
  measurement**: S05 records the pointer from the requirement to these exact fields
  (with the sha256 that pins them) instead of re-deriving them. R140's boundary is the
  same kind of statement — the typed absences stay typed, and the requirement says so by
  name rather than by prose.

- **S06 — Re-validate and re-issue the acceptance package.**
  Re-resolve all three thresholds against the frozen references, re-emit the acceptance
  package and readiness receipt, re-run the full gate. Proof: the three thresholds
  resolve to a real verdict; whatever verdict they give is reported as-is.

---

## 5. Explicitly out of scope

- No promotion, deploy, import, or graph write. All five locks stay closed.
- No change to `data/parse/acceptance_manifest.yaml` or the M513 release-decision package.
  The human release record is written by a human, never by an agent.
- No claim that the thresholds are *met*. This milestone makes them *measurable*. If the
  native plane measures below a frozen reference, that is the result and it is reported.
- No removal of the typed absences for transport surface, dialect flags, pdfalto stage or
  the visual lane unless S02 shows the lane is genuinely admissible.

---

## 6. Already closed (not this milestone's work)

| item | closed by |
|---|---|
| DB attribution gap for S08/S09 (validation Finding High) | `gsd_requirement_update` linked R123/R124/R133/R134/R135/R139 to `M514-1lss2s/S08` and `/S09` |
| da-cli example logging debt (HIGH preflight flag) | commit `ad2f6b905` — `init_pipeline_logging` in all six examples; every example in the crate now calls it |
| `data_integrity_hardcoded_url` false positives | commit `502a35292` — rule now skips inline `#[cfg(test)]` items, verified to mask zero production URLs |
| S08/T06 scorecard regression (3 red tests) | commit `102789d1e` (engine inactivity snapshot); `test_absorption_progress_governor.py` now green |

---

## 7. Risks and unknowns

**Synthesis of the 2026-10-03 spikes (S01–S05).** Every spike landed on the same shape:
validators flagged *missing explicit linkage*, while the mechanism or the evidence
already exists, is committed, and is already independently judged. In detail:

| slice | what the validator asked for | what the spike found |
|---|---|---|
| S01/S02 | a measurement engine for the frozen metrics | the producer exists (probe example, 4,762 lines); the acceptance runner hardcodes the gaps in `build_gap_ledger` |
| S03 | a same-n Java resource reference | per-document Java latency already recorded for 531 docs; the obstacle is pairing discipline |
| S04 | recompute + provenance checks | both already exist and are tested (`shared_producer_digest_is_unqualified`, `ProducerNotIndependent`) |
| S05 | conservation/contamination + reward versioning | already present (`reward_definition_sha256`, `corpus_digest`, `validation_gap`) |

This makes the milestone **predominantly a wiring and evidence-linkage milestone**, not a
build-from-scratch one. Estimate it that way. If any slice turns out to need genuinely
new mechanism, say so in the slice PLAN rather than absorbing the cost silently.

- **The thresholds may not be reachable.** `extraction_weakest_family`'s frozen reference
  is 0.4317 — a weakest-stratum value measured on the Java reference series. The native
  plane may genuinely measure lower. This milestone's success criterion is *measurability
  and honest reporting*, not passing.
- **`not_comparable` means absence of mechanism, not a near miss.** Do not treat S01/S02
  as tuning work.
- **Cost.** S02 is the "~2.5k-line measurement engine" M514 deferred. The 2026-10-03 spike
  (§4) reduced it to three named projections over the native route product — re-estimate
  from that list rather than from the line count.
- **Java stays active.** The reference series is produced by the Java route; this
  milestone does not retire it.

---

## 8. Definition of done

1. All three frozen thresholds resolve to `met | unmet` on the reference series, bound to
   the frozen reference by pointer, with the candidate never its own reference.
2. The same-n resource pair is measured, not typed absent.
3. R134/R140/R142 carry the explicit evidence their reviewers asked for.
4. The acceptance package re-issues with `s09_authorized` still *computed*, never declared,
   and `goal_unmet` reflecting the measured truth.
5. `governor architecture` and the full pytest/gate surface are green, with any pre-existing
   red declared by name in the validation record.
