# M517 independent review — capture and paired-policy evaluation

Date: 2026-10-13. Read-only review; no edits, cargo, tests, formatting or commit by reviewer. Review covers isolated capture, shared carriers, evaluator and frozen metric DATA. Parent owns execution and closeout.

## Verdict

One blocking implementation defect was found: successful TOC captures were being decoded as the GROBID port's ParsedArticle, although TOC sections have `depth`, not required `level`. It has now been fixed in the evaluator source by reading only the strict common text/identity view. No remaining winner/reward/native/denominator defect was found in the reviewed source. Two campaign/conclusion points below need attention before claiming comparable cycles or body-fidelity gains.

This is a source review, not proof of compilation or passing tests. The evaluator was still adding tests while review ran. Genuine TOC-output loading needs the planned offline integration regression and parent-owned test run.

## Findings

### 1. Blocking TOC schema mismatch — fixed in source, verify the real fixture

The original evaluator decoded every arm output as `da_ports::parser::ParsedArticle`. `build_pdf_toc_stage` serializes `PdfTocSection { depth, ... }`, while the port's `Section` requires `level`. Every successful actual TOC output would therefore fail before evaluation. New live capture JSON confirms `depth` and no `level`.

The current evaluator uses `RawArticle { paper_id, pdf_hash, sections: Vec<RawSection { text }> }` and checks TOC `parsed-article.v2`/`page_outline`, exact PDF SHA, paper ID, section text, evidence counts and successful nonempty output. This fixes the mismatch at the evaluator boundary without altering either parser/schema.

**Action:** execute the worker's planned offline `load_document` fixture using genuine `build_pdf_toc_stage` output, not a synthetic GROBID-shaped TOC fixture. It should pass source/observation/article hash checks and produce metrics for both successful arms. Existing microtests of tokens/metrics alone do not cover this boundary.

Source: `crates/da-cli/examples/policy_corpus/evaluation.rs`, `crates/da-application/src/pdf_toc_stage.rs`, `crates/da-domain/src/pdf_toc.rs:37–52`, `crates/da-ports/src/parser.rs:525–531`.

### 2. Important cycle comparability: report actual executable recipe identity

`metric_digest` currently hashes experiment config bytes, variant name and legacy metric bytes. Reports also bind captures/source/reference/registry. They do not bind the actual evaluator scoring implementation or actual capture executable. The manifest's `source_revision=718be28b5` predates the uncommitted new implementation. Correcting an evaluator bug later can change winner/reward results while leaving the metric_digest unchanged.

This matters for the planned per-document regression comparison under “unchanged metric”: same DATA is not evidence the scoring program is unchanged.

**Small action, no framework:** parent should freeze and record actual evaluator binary SHA and capture binary/recipe identity with the campaign, and compare cycles only under the same recorded executable/config/registry/capture identities. A parent-owned campaign provenance file is sufficient. Do not add a general registry/state subsystem. Captured article hashes already bind the observed result; retain them. If the scoring executable changes, re-evaluate all prefixes and identify the revision rather than calling the differences learning regressions.

Source: `evaluation.rs::run` metric digest/report fields; `artifacts/learning/policy-corpus-m517/manifest.json`.

### 3. Important conclusion scope: reference is full PDF; candidate is section text

The evaluator tokenizes only concatenated `sections.text` for each arm. The independent `pdftotext` reference is the full PDF. Bibliography, title/header/abstract, furniture and material projected into separate fields can therefore reduce section-text recall. In a live capture `2511.02200`, pdftotext ends in bibliography while GROBID holds 32 bibliography entries in `citations`, outside the scored sections. The TOC path can retain that material within section text. This difference is real for fulltext coverage but is not necessarily a semantic parse error.

**Action:** keep explicit `reference_scope=full PDF` and `candidate_scope=section text` in experiment interpretation. Report token/5-gram F1 as an independent PDF-text coverage diagnostic, not publisher ground truth, body-only fidelity, bibliography correctness or KG readiness. Do not broaden the oracle or adjust scores to improve one arm during the campaign. The current report already labels pdftotext diagnostic/not ground truth, which is correct; preserve the narrower scope in final conclusions.

Source: `evaluation.rs::article_metrics`, `policy_corpus_capture.rs::capture_reference`.

### 4. Native comparison is native DATA policy, not literal current CLI behavior

Both evaluations call actual `NativeFallbackPolicy` with identical measured per-document features and a frozen registry. The alias map for `xy_cut`/`grobid_structural` to captured Java structural output is explicit in experiment DATA/ADR. The native `page_outline` decision maps to the measured TOC arm.

However, existing `source_cli::run_parse_pdf` consults/stamps grammar then always executes GROBID; it does not dispatch the TOC arm when the grammar stamp says page_outline. Thus the counterfactual native-DATA policy comparison is rigorous for route decisions, but is not a measurement of the literal current CLI's production runtime/latency.

**Action:** label the baseline native DATA policy under the frozen execution map, not “current production routing” or “production speedup”. No production-route change is needed or authorized by this review.

Source: `source_cli.rs::run_parse_pdf`, `evaluation.rs::execution_index` / `select`, `parse_policy_experiment.yaml::native_execution_equivalence`.

## Checks that are sound in the reviewed source

- **Sources:** source PDF is hashed before capture, both raw-arm outputs carry matching PDF hash, input PDF/exporter/furniture hashes are rechecked after capture. Evaluator verifies PDF bytes, capture ID, paths, source hashes, article hashes, observed page/bookmark counts, reference hash/count and output/evidence counts.
- **Features:** real classifier result plus measured TOC bookmark count; classification failure remains `None` with error, not invented page0/quality0. Evaluator refuses missing/invalid features. TOC observations supply an independent page/bookmark/source consistency check.
- **Actual arms:** TOC exporter+`build_pdf_toc_stage` run directly into isolated directories. Java GROBID structural parser is called directly with release probe. Arm identity/profile is fixed and checked. No output is taken from production cache or a grammar stamp alone.
- **Isolation:** capture uses a new directory, immutable create_new artifacts, no production article/journal overwrites and no process-global journal-suppression env. Logging guard is added before runtime construction. Candidate-only locks are stamped and checked.
- **Latency:** TOC start precedes export/build/write. GROBID start precedes parser/probe/parse/write. Both record end-to-end arm wall time and matching evidence latency. Independent reference/classification setup is outside both arm clocks. This is an offline-capture latency diagnostic, not batch-server capacity.
- **Status:** only typed EmptyToc becomes NotApplicable. Export/parse/schema failures remain failed/error. Captures retain both statuses, errors and artifact evidence; evaluator keeps valid recorded failed arms as zero-reward outcomes rather than dropping them.
- **Splits:** one frozen manifest defines validation membership. Training-prefix growth does not re-rank the holdout. Unique IDs, unique PDF SHAs and unique capture directories are required across the whole manifest. One evaluation row per unique validation document.
- **Policy evaluation:** actual native-only and learned+native-fallback consults run on the same documents/features. Uncovered table cells continue through native. Selecting an unavailable arm remains unavailable with reward0, no fabricated reroute. Selected rewards/regret/all-document success use the same assigned-document denominator.
- **Accuracy:** decisive full-pair winner accuracy excludes forced/failed/tie cases. Forced successful rewards are separately reported. No observed-arm agreement is used as winner accuracy.
- **Rewards/regret:** winner1, loser/failed/NA0, successful tie0.5; max applicable reward minus selected reward. Both-failed regret0 is an oracle-relative statement, not good quality; operational failure count remains visible.
- **Text metrics:** repeated tokens are multiset counts; 5-token windows preserve local order; zero denominators are Option/null, not invented score1. Successful-only quality means have explicit n distinct from all-document reward/latency denominators.
- **Metric DATA:** exact legacy yield is frozen/read from current unchanged YAML; chars/ms and sections/ms alternatives are separately identified. QualityThenLatency uses configured quality tie margin0.02. No old metric digest is silently reinterpreted.
- **Support:** min_support4 is explicitly reported as arm observations (usually two independent documents for full pairs); distinct-document support is also reported. It is not confidence or a four-document guarantee.

## Live-capture inspection snapshot

This reviewer only read captures produced by parent; it did not run or recreate them. At the inspected snapshot:

- Captures inspected: 18
- Status pairs: {'not_applicable/success': 2, 'success/success': 16}
- Hash/page/bookmark/PDF/article/body-char/latency consistency findings: 0

The cohort is320fresh unique source hashes (240train,80validation), categories316arxiv/3textbook/1course. **Category is not applicability.** Actual early captures have many outlines. Do not infer NotApplicable from “arxiv” or from the historical corpus. A preliminary assumption sent by this reviewer was corrected explicitly after reading measured captures.

Current source checks are bound to this review snapshot:

- `crates/da-cli/examples/policy_corpus_capture.rs` SHA-256`a02cd2bc9466671da7fd09500a78f2e1d84e65aeb20ccc72397c44784e6a0906`
- `crates/da-cli/examples/policy_corpus/shared.rs` SHA-256`e8462571f3be72fdc1f737395a6a0edfc0af402516455b3f946b3722925ab4da`
- `crates/da-cli/examples/policy_corpus/evaluation.rs` SHA-256`d24bad34fde015ffaec6702fadf7a4ddc5cc920a8bbabf927d4d70d3cdde3823`
- `crates/da-cli/examples/policy_corpus_evaluate.rs` SHA-256`855b531fe4cd931075eea93fcd2fec0e386403078b32cb3be6d6ebfb3fcb365d`
- `data/policies/parse_policy_experiment.yaml` SHA-256`b7dc7fafd3832d95e7275335fb1f4e2dc20a3f1dc1ed21bc8e06ff047755b31e`

## Graph coverage and limits

GitNexus CLI query/context ran first, repo bound to daily-archive. New capture/evaluator symbols returned not-found. Several query rows contain NUL/malformed names/paths, so this graph result cannot prove caller completeness. Named parser/TOC contexts plus direct source confirmation were used to review the actual flows; no edit relied on graph output. No graph change analysis, cargo, subprocess invocation, real parser run or test execution by reviewer.

All recommendations preserve candidate-only locks and do not authorize import, graph writes, production promotion or unrelated tools/learning-critic work. No constants or files were changed by this read-only review.
