# Evidence — pitfalls

<!-- Written by one researcher role. Consumed by the decider. -->

Dimension: pitfalls
Questions assigned: Whether the 146 non-CI da-parse suites should join CI before any promotion milestone — mapper finding, owner steers later.

## Finding: GitHub CI runs only `cargo test -p da-parse --lib`; none of the da-parse integration suites execute there

- **Claim**: The sole workflow, `.github/workflows/architecture-guardrail.yml` (test step, "Unit + integration tests (per-crate, M339 + M432)"), runs `cargo test -p da-parse --lib` — the lib target only — while `da-application`, `da-adapters`, and `da-cli` run `--tests`. No step in the file invokes da-parse integration tests.
- **Source**: `.github/workflows/architecture-guardrail.yml` (read in full; the test step is the only place suites execute).
- **Confidence**: high
- **Why it matters here**: This is the exposure the INTENT names a known weak spot ("146 da-parse integration suites not in CI") and the residual risk of Risk 1; the question asks exactly what is unprotected.

## Finding: the in-repo architecture gate DOES run the da-parse integration suites since M514, and the recorded 23/23 PASS covers them

- **Claim**: `scripts/governor/__main__.py` (gate step list, lines ~777–782) runs `("da-parse tests", ["cargo", "test", "-p", "da-parse", "--tests"], t)` with an explicit comment: the old `--lib` step "ran only the unit tests, so every cross-module contract compiled and never ran". The recorded gate log `/tmp/arch-20261123-pass2.log` shows "da-parse tests... OK" inside "architecture gate steps: 23/23 passed, 0 timed out" → ARCHITECTURE GATE: PASS.
- **Source**: `scripts/governor/__main__.py:769-803` (read); `/tmp/arch-20261123-pass2.log` (read, tail).
- **Confidence**: high
- **Why it matters here**: There is no evidence gap about whether the suites pass at HEAD 0f06b764c — the recorded run is direct evidence (INTENT allows recorded run evidence). The gap is CI-only: a push to master that skips the local gate gets no da-parse integration signal from GitHub CI.

## Finding: the "compiled and never ran" failure mode is a trap this codebase has already fallen into once

- **Claim**: The M514 comment in the gate script documents that da-parse cross-module contracts previously compiled in CI/gate and never executed — a silent coverage hole caught only by human reading of the step definition. The CI workflow still carries the exact same `--lib` invocation for da-parse today.
- **Source**: `scripts/governor/__main__.py:777-782`; cross-checked against `.github/workflows/architecture-guardrail.yml`.
- **Confidence**: high
- **Why it matters here**: Brownfield pitfalls rule — the failure mode the research question worries about (suites rot because nothing executes them on push) is not hypothetical; it already happened and was fixed in the gate but not synced back to CI.

## Finding: suite-family inventory — 146 files, dominated by contract/oracle/property tests over committed fixtures

- **Claim**: `crates/da-parse/tests/` holds 145 top-level `.rs` files plus `parity_ledger/mod.rs` (= the 146 count). By suffix family: 79 `*_contract.rs`, 21 `*_oracle.rs`, 18 `*_properties.rs`, 7 `*_same_n.rs`, 1 `*_metamorphic.rs`, plus ledger/mutation suites (`grobid_parity_ledger_contract.rs`, `grobid_parity_ledger_mutations.rs`), semantic-corpus suites (`native_route_semantic_corpus.rs`), and the digest-only `native_visual_real_oracle.rs`. A `tests/fixtures/holdout/` tree carries 180+ committed TEI documents pinned by sha256 in `frozen_tables.json` with `provenance.yaml`.
- **Source**: directory listing of `crates/da-parse/tests/` and `crates/da-parse/tests/parity_ledger/`; `crates/da-parse/tests/fixtures/holdout/provenance.yaml`, `frozen_tables.json` present; suffix counts from listing.
- **Confidence**: high
- **Why it matters here**: These are exactly the R123-ladder-family and parity protections SC3/GROBID-state depend on; if promotion moves happen before CI executes them, promotion evidence rests on a hand-run gate log instead of a reproducible CI signal.

## Finding: cost signal — the da-parse suites are service-free and feature-gate-free in this crate

- **Claim**: `crates/da-parse/Cargo.toml` declares no `[features]` section and dev-dependencies are only `proptest`, `tokio (test-util)`, `serde*`, `serde_yaml`, `sha2`, `hex`, `roxmltree` — no service clients. Zero `#[ignore]` attributes are active in the suite files (the only mentions are comments saying the ignore pins are gone). HTTP suites (`backend_contract.rs`, `backend_termination_classification.rs`, `endpoint_proxy_contract.rs`, `cache_identity.rs`) spin in-process mock servers on `127.0.0.1:0` via `tokio::net::TcpListener` — no GROBID container or fd_api service is required. GROBID-related env names (`DA_GROBID_URL`, `DA_GROBID_MODEL_ROOT`) appear in tests as string constants policed by `architecture_contract.rs`, not as live calls.
- **Source**: `crates/da-parse/Cargo.toml` (read whole); grep over `crates/da-parse/tests` for `#[ignore`, `DA_GROBID|GROBID_URL`, `reqwest|TcpStream|http://`; `backend_contract.rs:88,457`, `endpoint_proxy_contract.rs:105,844`, `native_visual_real_oracle.rs:1-63` (read).
- **Confidence**: high
- **Why it matters here**: Closing the CI gap has no new infrastructure cost — no service containers, no feature-matrix expansion; the "cost" is only CI wall-time on the existing self-hosted runner. This contradicts any assumption that the 146 suites are the expensive, service-dependent part of the test estate.

## Finding: the genuinely service-dependent suites live outside da-parse and are already named/handled by the gate design

- **Claim**: The `*_real_model.rs` suites referenced by da-parse same_n/visual suite doc comments (`native_bibliography_same_n.rs:54`, `native_fulltext_same_n.rs:62`, `native_visual_family_same_n.rs:34`) live in `crates/da-adapters/tests/` (`native_header_real_model.rs`, `native_visual_family_real_model.rs`, `scalar_crf_real_model.rs`, whose own doc says the real-model resource is "explicitly missing or validated"). da-cli's example-driven tests are provisioned by an explicit seven-name list precisely because building all examples is expensive (~0.3 G per binary per the gate comment).
- **Source**: grep over `crates/` for `real_model|live_test`; `crates/da-parse/tests/native_bibliography_same_n.rs:54` et al.; `crates/da-adapters/tests/scalar_crf_real_model.rs:12`; `scripts/governor/__main__.py:784-800`.
- **Confidence**: high
- **Why it matters here**: The boundary of "what joining CI would cost" is correctly drawn at the gate script, not at the workflow: the expensive/live surface already has a documented provisioning strategy, and da-parse integration suites are on the cheap side of it.

## Finding: a measured preflight signal already exposes part of the gated-test exposure

- **Claim**: The recorded preflight `/tmp/preflight-20261122.log` scorecard reports `feature_gated_coverage: 19/33 (57.6%)` with "14 execute zero [attention]" — 14 feature-gated test targets execute zero tests under the feature sets the architecture gate actually builds. This is a same-shaped reliability hole one level up (gated targets), already instrumented by the governor scorecard.
- **Source**: `/tmp/preflight-20261122.log` (read, scorecard block).
- **Confidence**: high
- **Why it matters here**: If the owner steers toward "join CI before promotion", the same instrumentation pattern (scorecard axis) exists to track the 146-suite gap; the mechanism to measure closure is already in the repo.

## Finding: CI itself is already constrained to the self-hosted runner, so adding suites changes no runner topology

- **Claim**: The workflow header and checkout comment state the guardrail "runs only on self-hosted (vendor paths)" because Samyama/RuVector are path deps at `/root/vendor-source` (mapper evidence: `Cargo.toml` [workspace.dependencies]; workflow lines ~52–56).
- **Source**: `.github/workflows/architecture-guardrail.yml:44-56` (read in full); `.project/research/evidence-codebase.md` finding "builds are host-bound".
- **Confidence**: high
- **Why it matters here**: The change under consideration is a one-line edit to an existing self-hosted job (`--lib` → `--tests` for da-parse), not new CI infrastructure — the cost/tradeoff is runtime only.

## Finding: the INTENT already books the non-CI gap as an accepted residual risk, with a compensating control

- **Claim**: INTENT Risks states: "Static/graph verdicts may diverge from runtime behavior where CI does not execute (146 suites) — accepted as residual risk; every verdict carries a proof-weight tag". The compensating control exists (proof-weight tags + recorded gate log), meaning assessment reports are not silently wrong today — but promotion milestones would convert this residual risk into a production-facing one if CI never executes the suites.
- **Source**: `/root/daily-archive/.project/intent/INTENT.md` (Risks section, read).
- **Confidence**: high
- **Why it matters here**: Directly frames the decision: the risk is tolerable for an orientation milestone but its cost grows with what the suites are trusted to prove (promotion = SC3d gates).

## Finding: feature gates inside da-parse test files are absent; feature-gating lives in da-cli/da-adapters examples instead

- **Claim**: Grep for `feature = ` / `cfg(feature` across `crates/da-parse/tests/*.rs` finds no `#![cfg(feature)]` gate on any suite file (the only feature mentions are the da-cli example provisioning comments in the gate script); da-parse tests all build under default features, and the gate log's "feature-matrix build... OK" plus "da-cli feature-gated example runners/contracts... OK" steps carry the gated surface.
- **Source**: grep over `crates/da-parse/tests` for feature patterns (no hits in suite files); `/tmp/arch-20261123-pass2.log` step list; `scripts/governor/__main__.py:784-803` (`policy_ablation_artifact_test` note, learning-evaluation gating).
- **Confidence**: medium
- **Why it matters here**: Confidence is grep-level, not a per-file read of all 146. A `--tests` switch for da-parse in CI cannot silently skip suites behind un-built features — the whole 146 run as one ungated unit, which makes the CI fix honest and complete.

## Assigned questions — answers

- Whether the 146 non-CI da-parse suites should join CI before any promotion milestone — mapper finding, owner steers later. → Facts (the should-join decision belongs to decide): the 146 suites (79 contract / 21 oracle / 18 properties / 7 same_n / metamorphic / ledger / semantic-corpus over sha256-pinned committed TEI fixtures) already execute in the in-repo architecture gate recorded 23/23 PASS at /tmp/arch-20261123-pass2.log but NOT in GitHub CI, whose da-parse step is still --lib (.github/workflows/architecture-guardrail.yml); what is exposed is the R123-ladder and GROBID-parity protection any promotion milestone would rely on, trusted only where the governor gate is run by hand — the "cross-module contract compiled and never ran" failure mode already occurred once and was fixed in the gate, not CI (scripts/governor/__main__.py:777-782), so the two entry points have drifted and stay drifted; cost of closing is no new infrastructure (self-hosted runner already required by vendor path deps, suites carry no feature gates, no #[ignore], no external services — GROBID/fd_api mocks are in-process TcpListeners, live suites live in da-adapters/da-cli) — a one-line --lib to --tests switch whose only cost is CI wall-time, trackable via the existing feature_gated_coverage preflight axis (19/33, /tmp/preflight-20261122.log); INTENT frames the gap as accepted residual risk for M001 with proof-weight tags as compensating control, converting to production-facing exactly when promotion milestones start trusting these suites (SC3d gates). Full finding-level detail above; family breakdown and dead ends recorded below.

## Dead ends

- `crates/da-parse/tests/` grep for `#[ignore` — no active ignored tests; only comments stating ignore pins were removed. Not a factor in the CI decision.
- Grep for `fd_api|:8000` in da-parse tests — hits are all fixture TEI prose or unrelated strings; no fd_api service dependency exists in da-parse suites.
- Searching da-parse tests for live-service env gates (`env::var("GROBID…")` execution paths) — hits are the string constants policed by `architecture_contract.rs` (fascade-shell surface markers), not runtime service calls; no service-wait/skip scaffolding exists to worry about in CI.
