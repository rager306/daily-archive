# GROBID state report (M001 orientation/assessment)

Assessment snapshot at branch `gsd-path/M001`, HEAD `0f06b764c`. Method: static reading +
codebase-memory-mcp + recorded logs only (`/tmp/arch-20261123-pass2.log`, `/tmp/preflight-20261122.log`);
no new cargo/test runs were performed for this report. This is assessment only: no gate flip, no pin,
no threshold, no status value changes (N1 — diff is .md-only). Verdict vocabulary: работает /
кандидат / заглушка / unverified, each with a proof-weight tag (ran-in-gate / static-only / unverified).

## (a) Production route + identity

The production parse route is the GROBID Java service over HTTP; the native Rust route is a
candidate wave, not deployed.

- **Route surface** [ran-in-gate]: the production path delegates through `grobid_parse` in
  `crates/da-parse/src/parser.rs`; the HTTP backend is `crates/da-parse/src/http.rs`
  (`DEFAULT_GROBID_URL = http://127.0.0.1:8070`, backend id `grobid-http` from
  `crates/da-parse/src/backend.rs`, `GrobidParser` struct at `crates/da-parse/src/http.rs` line 838).
- **Identity [ran-in-gate]**: current identity is GROBID 0.9.1, pinned by image digest in
  `.env.example` (`GROBID_IMAGE=grobid/grobid@sha256:2239…4cd`, `GROBID_CONTAINER_NAME`,
  `GROBID_JAVA_OPTS`, `GROBID_AUTO_START`, `GROBID_START_TIMEOUT_SECONDS`; `GROBID_URL` /
  `GROBID_HOST_PORT` / `DA_GROBID_URL` keys at `.env.example:118-131`); the pin is enforced by
  `crates/da-cli/tests/grobid_release_identity_test.rs` (per the comment in `.env.example:156`).
  Identity D576 ("GROBID 0.9.1 = current identity") and rulings D572–D575 carry per
  `.project/intent/INTENT.md` [documentary]. Rollback target: GROBID 0.9.0-crf [documentary, INTENT].
- **Model fingerprint [static-only]**: `grobid_model_fingerprint(version)` in
  `crates/da-parse/src/http.rs` line 97 maps the version string to a typed model fingerprint.
- **Native route is a candidate, never production [ran-in-gate]**: the acceptance package locks read
  `production_route=forbidden`, `deployed=false`, `auto_promote=forbidden`, `graph_write=forbidden`,
  `import_eligible=false`, `thresholds=not_accepted`, `s09_authorized=false`
  (`artifacts/parse_compat/grobid_retirement_acceptance_package.json`, `locks` and `authorization`).
  The readiness receipt carries `claim_scope: blocked_pending_human_authorization`
  (`artifacts/parse_compat/grobid_retirement_readiness.json`).

## (b) Native ladder family table (R123 set)

The family set is exactly the R123 ladder (lattice/segmentation, header, bibliography, body
accounting, visual). Execution plane of record: **no da-parse integration suite executes in GitHub
CI** — the CI workflow runs `cargo test -p da-parse --lib`
(`.github/workflows/architecture-guardrail.yml` line 101), so all same-n/integration suites below run
only under the local governor gate, recorded 23/23 PASS (`/tmp/arch-20261123-pass2.log`,
`architecture gate steps: 23/23 passed, 0 timed out`; `ARCHITECTURE GATE: PASS`). No suite below is
CI-protected. Also verified: no `#![cfg(feature)]` gate exists on any
every file under `crates/da-parse/tests` (glob *.rs) file (grep over the directory at HEAD finds only the comment in
`crates/da-parse/tests/native_retirement_authorization_contract.rs` line 13 stating the test is
deliberately ungated) — the 146 suites execute as one ungated unit.

Statuses: no family row below claims "promoted"; no gate receipt for a promotion exists in the
recorded evidence, so every family stays кандидат (N2).

| Family | Status | Verdict | Evidence | Proof weight |
|---|---|---|---|---|
| Lattice / segmentation | кандидат | кандидат | `crates/da-parse/tests/native_segmentation_oracle.rs`, `crates/da-parse/tests/native_segmentation_contract.rs`, `crates/da-parse/tests/layout_lattice_contract.rs`, `crates/da-parse/tests/layout_lattice_data_contract.rs` | ran-in-gate (governor 23/23), not CI-executed |
| Header | кандидат | кандидат | `crates/da-parse/tests/native_header_same_n.rs`, `crates/da-parse/tests/native_header_native_input_contract.rs`, `crates/da-parse/tests/native_header_native_input_oracle.rs` | ran-in-gate (governor 23/23), not CI-executed |
| Bibliography | кандидат | кандидат | `crates/da-parse/tests/native_bibliography_same_n.rs`, `crates/da-parse/tests/native_bibliography_contract.rs`, `crates/da-parse/tests/native_bibliography_native_input_oracle.rs` | ran-in-gate (governor 23/23), not CI-executed |
| Body accounting | кандидат (axis gate not assessable) | кандидат | code `crates/da-parse/src/native_body.rs`, labels `data/parse/native_body_accounting.yaml`; suites `crates/da-parse/tests/native_body_accounting_contract.rs`, `crates/da-parse/tests/native_body_uncertainty_split_contract.rs`; residual axis `artifacts/parse_compat/native_body_uncertainty_split.json` | ran-in-gate (governor 23/23) for code suites; the axis-level verdict is `not_assessable` — no independent body-localization oracle exists in tree (see §(d)) |
| Visual | кандидат | кандидат | `crates/da-parse/tests/native_visual_family_same_n.rs`, plus per-kind same-n suites `native_visual_figure_same_n.rs`, `native_visual_table_same_n.rs`, `native_visual_formula_same_n.rs`, typed-absence suite `native_visual_typed_absence.rs` | ran-in-gate (governor 23/23), not CI-executed |

Typed-absent rows (E6): none — every R123 family has code and suites in tree; no family is
silently skipped.

Beyond-set findings (E5) — suites found outside the R123 family set, each recorded as its own
кандидат row, never folded into the ladder:

| Beyond-set area | Status | Verdict | Evidence | Proof weight |
|---|---|---|---|---|
| Callout matching / classification | кандидат | кандидат | `crates/da-parse/tests/callout_matching_contract.rs`, `crates/da-parse/tests/native_callout_classification_contract.rs` | ran-in-gate (governor 23/23), not CI-executed |
| Fulltext composition | кандидат | кандидат | `crates/da-parse/tests/native_fulltext_same_n.rs`, `crates/da-parse/tests/native_fulltext_composition_contract.rs` | ran-in-gate (governor 23/23), not CI-executed |
| Java-lift oracle harness | кандидат (harness, not family) | кандидат | `crates/da-parse/tests/java_lift_same_n_oracle.rs` | ran-in-gate (governor 23/23), not CI-executed |

The java→native shadow join plane (`crates/da-parse/src/p5_shadow_join.rs`) is the mechanism that
keeps native output comparable against the GROBID production bytes while the route stays
shadow/candidate [static-only].

## (c) Retirement-package status

- **Acceptance package** `artifacts/parse_compat/grobid_retirement_acceptance_package.json`
  [ran-in-gate, artifact-internal evidence]: 21 mandatory gates (states: 6 met, 12
  `required_family_downgrade` recorded `not_applicable`, 1 `accepted_residual`, and the four
  blocking states below), `claim_scope` blocked, locks per §(a).
- **Readiness receipt** `artifacts/parse_compat/grobid_retirement_readiness.json`:
  `claim_scope: blocked_pending_human_authorization` with four typed refusal codes
  (`human_reviewer_unnamed`, `human_decision_not_recorded`, plus two `required_gate_blocks`
  instances of the same vocabulary over different gates) [ran-in-gate, receipt-derived].
- **Source binding** [static-only]: `crates/da-cli/src/native_retirement_readiness.rs` carries
  `READINESS_EVIDENCE_CARRIERS` (5 `(path, pointer)` carriers, `:94`); both the producing example
  and the contract test derive their sha256 from this one list, so the bound set cannot drift
  (`crates/da-cli/examples/native_retirement_readiness.rs`,
  `crates/da-cli/tests/native_retirement_readiness_contract_test.rs`).
- **Authorization contract** [static-only]: `authorize_native_retirement` at
  `crates/da-parse/src/native_retirement_authorization.rs` line 314 decides the authorization verdict
  fail-closed; its deliberately ungated contract suite is
  `crates/da-parse/tests/native_retirement_authorization_contract.rs`.
- Nothing in this milestone flips any of these [N1/N2].

## (d) Gate inventory — what separates current state from any production flip

Source of truth: `artifacts/parse_compat/grobid_retirement_acceptance_package.json`
(`mandatory_gates`, 21, + `blocking_gates`, 4). All four blocking ids are marked **BLOCKING**.
Feasibility notes follow the settled RQ1 classification and are **judgments, not measurements**.

Blocking gates:

1. **BLOCKING `body_axis.body_not_assessable`** (`not_assessable`): needs an independent
   body-localization oracle that decides `p1_localization_uncertain` on the frozen corpus.
   Feasibility: **new measurement infrastructure required** — no independent body-localization
   oracle exists in the tree; this is the one gate that cannot be closed by running existing
   packages.
2. **BLOCKING `named_gap_learning_ruvllm_ablation`** (`typed_refusal`, `D471:no_java_donor`):
   needs a Java donor for `learning.ruvllm_ablation` **or an explicit owner scope decision**
   (D471 stays fail-closed typed_refusal). Feasibility: two readings recorded as roadmap-input,
   not resolved — a Java-donor ablation build, or an owner scope decision.
3. **BLOCKING `h20_frozen_external_holdout_decision`** (`pending`): needs a recorded human
   release decision — named reviewer, explicit decision, passing quality/cost/rights gates
   (manifest `doc/review/grobid-absorption-followup.json`, pointer `stages`). Feasibility:
   execution of existing packages (`scripts/governor/adapters/checks/external_holdout_package.py`
   machinery) **plus a human act** — the machinery runs, the decision must be made and recorded.
4. **BLOCKING `learning.s07_measured_no_gain`** (`unmet`): needs a learning candidate that
   achieves a same-n gain over the frozen corpus (manifest `artifacts/parse_learning/candidate_cycle.json`,
   pointer `outcome`). Feasibility: execution of existing packages — the learning cycle machinery
   `crates/da-application/src/parse_learning_cycle.rs` runs; closing needs a candidate that wins.

Also blocking-adjacent, non-blocking state: **`h21_revision_label_residual`**
(`accepted_residual`, manifest `artifacts/parse_compat/grobid_retirement_release_decision_package.json`,
pointer `remainders`) — execution of existing packages suffices to re-measure; currently a
recorded residual, not a blocker.

The remaining 16 mandatory gates are all execution-of-existing-packages:

- Threshold gates, state met: `threshold.preservation_micro_occurrence`,
  `threshold.extraction_micro_occurrence`, `threshold.extraction_weakest_family` — measured over
  manifest `artifacts/parse_compat/native_corpus_acceptance.json` (pointer `documents`).
  Re-execution path: same-n suites + conservation receipts
  (`crates/da-application/src/native_family_promotion_gate.rs`,
  `crates/da-application/src/parse_learning_cycle.rs`,
  `crates/da-application/src/context_conservation.rs`).
- `required_scope.*` downgrade gates (12), state `not_applicable` by recorded downgrade:
  `required_scope.rest.{isalive,health,version,processFulltextDocument,processHeaderDocument,processReferences}`,
  `required_scope.dialect.teiCoordinates`,
  `required_scope.stage.{pdfalto_layout,document_segmentation,figure_table_formula,bibliographic_consolidation,tei_lift}`
  — manifest `data/parse/absorption_progress.schema.yaml`, pointer `required_scope.families`;
  read by the governor absorption-progress check (`scripts/governor/checks/grobid_absorption_progress.yaml`,
  reader `scripts/governor/adapters/absorption_progress_reader.py`).
- `resource.container_java_cpu`, state met — measured same-n resource pair
  (`artifacts/parse_compat/native_corpus_resource_same_n.json`, pointer `reference_metrics`).

Dual-metric / val-gap machinery that these executions bind through:
`crates/da-cli/examples/policy_corpus/evaluation.rs` line 935 and
`crates/da-cli/src/native_acceptance_package.rs` line 138; the R135/R130 derived-evidence maturity
classifier lives at `crates/da-application/src/derived_evidence_maturity.rs`. All static-only
pointers, cited as mechanism locations, not as measurements.

### Preflight axes (dated, stale snapshot)

Recorded 2026-11-22 in `/tmp/preflight-20261122.log`; the log **predates the M001 doc edits**, so
these numbers are a snapshot, not current measurements. Quoted verbatim:

- `axis measurability: 5/21 (23.8%) — mandatory acceptance gates that carry a measured mechanism
  over all mandatory gates.`
- `axis feature_gated_coverage: 19/33 (57.6%) — gated test targets that execute a non-zero number
  of tests over all test targets carrying a cfg feature gate, under the feature sets the
  architecture gate actually builds`.

The feasibility notes above are judgments, not measurements.
