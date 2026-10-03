# Next milestone spec — DRAFT for human review

**Status:** DRAFT. Not created. Awaiting owner review before `gsd new-milestone`.
**Drafted:** 2026-10-03, after M514-1lss2s was parked with `needs-attention`.
**Scope source:** recorded evidence only. Nothing here is invented; every item cites a
finding that a validator already produced.

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

- **S03 — Same-n Java resource reference.**
  Retires `resource.container_java_cpu = not_observed{reference_row_not_same_n}` by
  producing a same-n Java CPU reference for the paired resource envelope. Proof: the
  operational axis reports a measured pair instead of a typed absence, and
  `pairing=same_documents_only` / `pooled=forbidden` stay enforced.

- **S04 — Independent recompute and provenance evidence (R134).**
  Adds the explicit reviewer-recomputability-from-readable-source-bytes check and the
  same-generator/different-label provenance check the R134 reviewer found absent. Proof:
  both checks fail closed on an injected mutation and pass on the committed carriers.

- **S05 — R140 / R142 evidence closure.**
  Makes the conservation/contamination measurement and reward versioning R142 demands
  explicit, and re-states R140's delivered-vs-typed-absence boundary with measured
  numbers. Proof: the named measurements are present and pinned.

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
