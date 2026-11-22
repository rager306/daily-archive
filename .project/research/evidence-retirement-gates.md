# Evidence — retirement-gates

<!-- Written by one researcher role. Consumed by the decider. -->

Dimension: retirement-gates
Questions assigned: Which of the retirement gates (SC3d list) require new measurement infrastructure versus only execution of existing packages — feeds the roadmap after M001.

Method note (per INTENT Constraints): codebase-memory/graph-index evidence was combined with static file reading and recorded logs only — no cargo/build/test runs were executed for this research. Sources opened: repo paths at the working tree of branch gsd-path/M001, `/tmp/arch-20261123-pass2.log`, `/tmp/preflight-20261122.log`, and `.project/research/evidence-codebase.md`.

## Finding: the SC3d gate vocabulary maps onto a concrete, already-coded gate set in the acceptance package

- **Claim**: The S08 acceptance package `artifacts/parse_compat/grobid_retirement_acceptance_package.json` (`mandatory_gates`, read at lines 199–523) enumerates the machine-checkable retirement gate set: `threshold.extraction_weakest_family` (met), `body_axis.body_not_assessable` (not_assessable, blocks), `named_gap_learning_ruvllm_ablation` (typed_refusal, blocks), `h20_frozen_external_holdout_decision` (pending, blocks), `h21_revision_label_residual` (accepted_residual, non-blocking), `learning.s07_measured_no_gain` (unmet, blocks), plus resource-pair and required-scope rows (met/not_applicable). A `blocking_gates` block repeats exactly the four blocking rows with per-gate `evidence_needed` strings.
- **Source**: `artifacts/parse_compat/grobid_retirement_acceptance_package.json:434-523` (also the acceptance-package contract `crates/da-parse/tests/grobid_retirement_package_contract.rs`, which fail-closed-pins the gate ids including `named_gap_learning_ruvllm_ablation` at line 156).
- **Confidence**: high
- **Why it matters here**: SC3d's "exhaustive list of gates" and this research question can be answered against a committed artifact rather than prose — the infra-vs-execution classification below keys off these exact gate ids.

## Finding: the blocked-readiness receipt machinery is complete, executed, and source-bound

- **Claim**: `crates/da-cli/src/native_retirement_readiness.rs` (770 lines, S09/T02) is a pure-std module that binds five evidence carriers by repository-relative path + JSON pointer + sha256 (`READINESS_EVIDENCE_CARRIERS`, lines 84–110: acceptance package, `data/parse/acceptance_manifest.yaml` human-release record, absorption-progress schema required-scope + authorization blocks, M513 release-decision package), emits `artifacts/parse_compat/grobid_retirement_readiness.json`, and fail-closed-asserts the blocked state (`goal_unmet = true`, `families_switched = []`, human decision `pending`, body residual token `p1_localization_uncertain` verbatim, five locks intact). Its docstring states it "performs no filesystem, network, clock, RNG or evaluator access" for the module itself; the ungated example `crates/da-cli/examples/native_retirement_readiness.rs` reads the committed carriers and writes the artifact, and the contract test `crates/da-cli/tests/native_retirement_readiness_contract_test.rs` "re-derives every pin from the committed bytes".
- **Source**: `crates/da-cli/src/native_retirement_readiness.rs:1-123`; produced artifact `artifacts/parse_compat/grobid_retirement_readiness.json` (present, 306 lines, with `claim_scope: blocked_pending_human_authorization` and the four refusal codes at lines 239–303).
- **Confidence**: high
- **Why it matters here**: this is the R136 source-bound receipt layer — it already exists, has run (artifact committed), and needs no new infrastructure.

## Finding: conservation receipts exist at three layers (family gate, learning cycle, assurance)

- **Claim**: Conservation vetoing/receipting is implemented in (a) `crates/da-application/src/native_family_promotion_gate.rs` (`conservation_veto` field, line 161; fail-closed ordering at line 319), (b) `crates/da-application/src/parse_learning_cycle.rs` (`measure_conservation` recomputes the corpus digest and replays the RVF journal, lines 360–450; `#[error("conservation was vetoed")]` line 181), and (c) `crates/da-application/src/context_conservation.rs` / `assurance.rs` (`observe_conservation_evidence`, `ConservationEvidence`, byte_conservation receipts), with policy DATA at `data/ontology/context_conservation_policy.yaml` loaded by `crates/da-adapters/src/context_conservation_policy.rs`. The `native_family_same_n_adapter.rs` refuses any family whose `conservation_ok = false` or `conservation_veto = true` (lines 198, 342).
- **Source**: `crates/da-application/src/native_family_promotion_gate.rs`, `crates/da-application/src/parse_learning_cycle.rs`, `crates/da-application/src/context_conservation.rs`, `crates/da-parse/tests/architecture_contract.rs:6567-6611` (single-evaluator + quarantine checks on the same-n adapter).
- **Confidence**: high
- **Why it matters here**: the "conservation receipts" SC3d gate is execution of existing packages, not new measurement infrastructure.

## Finding: the same-n comparison layer is an extensive existing suite family

- **Claim**: Dedicated same-n suites exist per ladder family: `native_header_same_n.rs`, `native_bibliography_same_n.rs`, `native_fulltext_same_n.rs`, `native_visual_family_same_n.rs`, `java_lift_same_n_oracle.rs`, plus same-n assertions inside `native_date_contract.rs`, `native_affiliation_contract.rs`, `native_personal_name_oracle.rs`, `native_citation_oracle.rs`, and shared denominators pinned in `data_contracts.rs` (`same_n_required`, `same_n_local_proxy_required`). These are part of the 146 da-parse integration files that CI does not execute (settled mapper finding).
- **Source**: grep over `crates/da-parse/tests/` for `same_n` (files listed above); `.project/research/evidence-codebase.md` finding "the largest integration-test surface (da-parse, 146 files) is not run in CI".
- **Confidence**: high
- **Why it matters here**: same-n comparison = execution of existing packages; the only infra-adjacent risk is CI coverage, which is the pitfalls dimension's question, not a new measurement need.

## Finding: dual task metrics and val_gap measurement machinery already exist

- **Claim**: Task-metric measurement exists as same-n pair carriers (`crates/da-cli/examples/support/native_semantic_measure.rs`, `crates/da-cli/examples/support/native_threshold_measure.rs`, both reading `external_holdout_contract.release_probe` from `data/parse/acceptance_manifest.yaml`) and as learning-loop metrics with an explicit validation gap: `crates/da-cli/examples/policy_corpus/evaluation.rs:935` computes `val_gap_train_minus_validation_reward`; `parse_policy_ablation`/`parse_learning_evaluation` in `crates/da-application/src/` carry `maximum_validation_gap` (example 0.05, `crates/da-cli/examples/parse_policy_ablation.rs:45`) and `conservation_veto` in the same fail-closed clause set; `crates/da-cli/src/native_acceptance_package.rs:138` pins `POINTER_LEARNING_VALIDATION_GAP = "validation_gap"`. The M515 coverage reconciliation states the delivered contour verbatim: "train -> persist -> reload -> independent eval on one frozen set, dual metrics plus same-n and validation gap; … no auto-promotion" (`artifacts/parse_compat/m515_requirement_coverage_reconciliation.json:103,130`).
- **Source**: paths above; `artifacts/parse_compat/m515_requirement_coverage_reconciliation.json`.
- **Confidence**: high
- **Why it matters here**: "dual task metrics" and "val_gap" from SC3d are existing measurement machinery — their gate cost is running them, not building them.

## Finding: explicit promotion authorization is a human-record gate with fail-closed code already in place

- **Claim**: `authorize_native_retirement` (da-parse, exercised by `crates/da-parse/tests/native_retirement_authorization_contract.rs`, S09/T01) computes a typed verdict and refuses unless the human release record names a reviewer and records an authorizing decision: the produced readiness artifact lists refusals `human_reviewer_unnamed` ("a blank or absent reviewer is not a named human") and `human_decision_not_recorded` ("decision `pending` is not a recorded authorizing token (recorded, approved)") (`artifacts/parse_compat/grobid_retirement_readiness.json:247-256`). The acceptance package's H20 row defines exactly what must be recorded: "a recorded human release decision: named reviewer, explicit decision and passing quality/cost/rights gates" (`grobid_retirement_acceptance_package.json:439-451`).
- **Source**: `crates/da-parse/tests/native_retirement_authorization_contract.rs:1-60,164-260`; `artifacts/parse_compat/grobid_retirement_readiness.json:239-303`; `artifacts/parse_compat/grobid_retirement_acceptance_package.json:439-451`.
- **Confidence**: high
- **Why it matters here**: the authorization gate needs zero new code — it needs the owner's recorded decision, which INTENT hard-locks as out of scope for M001 (auto-promote/production_route flip forbidden).

## Finding: evidence-maturity checking (R135/R130) exists as a 1,557-line classifier

- **Claim**: `crates/da-application/src/derived_evidence_maturity.rs` implements evidence-maturity states with blocked reasons including `ConservationLoss` (line 220 maps it to `"conservation_loss"`; line 812 turns the flag into `BlockedReason::ConservationLoss`) and a full-receipt-set outcome that returns `HumanGateRequired` directly (documented in `doc/review/grobid-absorption-review-2026-09-10.jsonl:7`, standards `["R130","R118","ADR-059"]`, noting "Full receipt set returns HumanGateRequired directly; ReviewReady is not derived"). R136 command/freshness provenance is carried by the same sha256 path+pointer binding pattern (`crates/da-cli/src/native_family_receipt.rs` `CarrierBinding`, used by the readiness module).
- **Source**: `crates/da-application/src/derived_evidence_maturity.rs`; `doc/review/grobid-absorption-review-2026-09-10.jsonl:7`; `crates/da-cli/src/native_retirement_readiness.rs:29-56`.
- **Confidence**: high
- **Why it matters here**: R135/R130/R136 gate infrastructure is fully present; classification is "execution only".

## Finding: governor-side retirement checks are DATA-rule based and already exercised

- **Claim**: `scripts/governor/adapters/checks/external_holdout_package.py` checks external-holdout package contracts including invalidation rules (`invalidation_rule_missing`) and the retired-result rule (`retired_result_citable`, `retired_result_may_be_cited is False`, lines 397–402); `scripts/governor/adapters/checks/grobid_absorption_progress.yaml` pins positive/negative fixtures for `native_retirement_readiness_blocked`/`_broken` (lines 1313–1318), exercised by `scripts/governor/tests/test_absorption_progress_retirement_readiness.py`.
- **Source**: `scripts/governor/adapters/checks/external_holdout_package.py:50-60,397-402`; `scripts/governor/adapters/checks/grobid_absorption_progress.yaml:1313-1318`.
- **Confidence**: high
- **Why it matters here**: the governance/checking surface for the gates already exists; no new check infrastructure is needed.

## Finding: recorded run evidence covers the gate example surface but the parse suites themselves are not CI-recorded

- **Claim**: The recorded full architecture gate log `/tmp/arch-20261123-pass2.log` contains exactly one retirement-surface line ("da-cli content-receipt example runner... OK", line 21) plus the overall 23/23 pass — it does not record execution of the da-parse retirement contract suites, consistent with the settled finding that CI runs `cargo test -p da-parse --lib` only. The preflight log `/tmp/preflight-20261122.log` reports `axis measurability: 5/21 (23.8%)` — "mandatory acceptance gates that carry a measured mechanism" — i.e. most mandatory gates were, at preflight time, not in a measured state.
- **Source**: `/tmp/arch-20261123-pass2.log:21`; `/tmp/preflight-20261122.log:2`; `.project/research/evidence-codebase.md` (da-parse CI finding).
- **Confidence**: high
- **Why it matters here**: INTENT allows only recorded evidence, and the recorded evidence base for "execution of existing packages" is thin for the da-parse retirement suites — a post-M001 roadmap item must decide whether executing them (with recorded logs) counts as sufficient, which is exactly the pitfalls-dimension CI question.

## Assigned questions — answers

- Which of the retirement gates (SC3d list) require new measurement infrastructure versus only execution of existing packages — feeds the roadmap after M001. → Gate-by-gate (ids from artifacts/parse_compat/grobid_retirement_acceptance_package.json blocking/mandatory ledger): only body_axis.body_not_assessable needs new measurement infrastructure — an independent body-localization oracle deciding p1_localization_uncertain on the frozen corpus (evidence_needed verbatim at json:216-231, blocking_gates[0]; no such oracle exists in the tree, body uncertainty is only split via native_body_uncertainty_split.json); named_gap_learning_ruvllm_ablation needs either a Java-donor ablation build (new infrastructure, contradicts retirement direction) or an owner scope decision on D471 (typed_refusal wired at crates/da-cli/src/native_acceptance_package.rs:528-531); learning.s07_measured_no_gain is execution of the existing parse_learning_cycle loop plus an open research outcome (a winning same-n candidate over the frozen corpus) or an accepted-residual owner ruling — not new infrastructure (ADR-080 already concluded measured_no_gain, commit 0fcd1546a); h20_frozen_external_holdout_decision is execution (capture/probe refresh via crates/da-cli/examples/external_holdout_capture.rs + external_holdout_release_probe.rs, governor external_holdout_package.py) plus an unrecorded human release decision; conservation receipts, same-n comparison, dual task metrics, val_gap, R136 source-bound receipts and R135/R130 evidence maturity are all execution of existing packages (native_family_promotion_gate.rs conservation_veto, parse_learning_cycle.rs measure_conservation, native_{header,bibliography,fulltext,visual_family}_same_n.rs, crates/da-cli/examples/support/native_semantic_measure.rs + native_threshold_measure.rs, policy_corpus/evaluation.rs:935 val_gap, native_family_receipt.rs CarrierBinding, derived_evidence_maturity.rs 1,557 lines); explicit promotion authorization is a pure recorded human action (data/parse/acceptance_manifest.yaml release_decision_package.human_release_review) — hard-locked out of M001 by INTENT; practical gate on all execution items: the decisive da-parse retirement suites do not run in CI (cross-reference the pitfalls dimension).

## Dead ends

- `val_gap` / `dual_task` keyword search inside `crates/da-parse/tests/` — no matches; the val_gap/dual-metric machinery lives in `crates/da-application/src/parse_learning_*`, `crates/da-cli/examples/` (policy_corpus, parse_policy_ablation, support/), and `crates/da-cli/src/native_acceptance_package.rs`. Searching the parse-test tree for these names is unproductive.
- `/tmp/arch-20261123-pass2.log` as evidence of retirement-suite execution — contains only the overall gate result and "da-cli content-receipt example runner... OK"; it does not record the da-parse retirement contract suites, so it cannot serve as per-gate execution evidence.
- `/tmp/preflight-20261122.log` for per-gate readiness detail — reports only aggregate axes (`measurability 5/21`); useful as a stale-state snapshot, not a gate-by-gate source.
- Graph-side verification via codebase-memory `search_graph`/`trace_path` for the gate symbols — not additionally pursued: plain file reading already yielded exact paths, line numbers, and constants for every gate; re-deriving the same facts through the graph would add no decision-relevant information. (Recorded here per the honesty rule: this researcher relied on direct file reads; no contradiction with the settled mapper evidence was found.)
- `crates/da-parse/tests/fixtures/holdout/*.tei.xml` "retirement" hits — false positives (prose mentioning retirement communities in corpus documents), not gate code.
