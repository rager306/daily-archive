# Parse-policy evaluation audit

Date: 2026-10-13. Scope: read-only audit of `feat/da-parse`; no Rust edits, cargo, graph writes, promotion, or metric adoption. Candidate-only locks unchanged.

## Verdict

The ADR-080 claim “61.1% validation — first measurable gain” is unsupported. Its numerator counts agreement with the recorded action codes of **both** arms, not agreement with the winner. More forced one-arm records can increase that number without improving a policy. The four-feature table may still be useful; a corrected same-document evaluation is required before a gain/no-gain claim.

## Evidence and graph coverage

GitNexus CLI query/context was run before source inspection. Repository bound to `daily-archive`. The index reported `1c0a5a5` versus current `718be28`, 67 commits behind; parent is rebuilding it. Queries found native-policy flows, but contexts for `table_accuracy`, `table_mean_reward`, `score_pair`, `score_gold_pair`, `rouge_l`, and `split_observations` returned not-found. Those are unresolved graph results, not “unused” evidence. A workspace Rust source scan confirmed the functions and `main.rs` consumers listed below. No edit depends on a stale impact walk.

Code references are to the audited main worktree. Arithmetic examples below independently calculate the consequence of the inspected recurrence/data; they are **not** Rust runtime tests.

## Findings

### 1. Observed-action agreement is not pair-winner accuracy (blocking)

- `crates/da-application/src/parse_policy_table.rs:295–315`: `table_accuracy` increments coverage per observation and agreement when selected code equals `obs.decision_code`. It never looks at `reward` or verdict.
- `crates/da-cli/src/main.rs:1550–1564`: `policy learn` labels that result “validation accuracy”.
- `parse_policy_pair.rs:290–309`: every full pair emits winner and loser observations with the **same document/features**, and one code each.

For a full pair, any covered table selecting one of the two measured codes earns 1/2 agreement whether it picks winner or loser. A forced applicable-arm observation earns 1/1. Thus action agreement reflects the full/forced composition, not winner prediction.

Historical data in `/root/worktrees/e1-critic` reconstructs the ADR numbers exactly:

| Item | Count |
|---|---:|
| Training observations | 247 |
| Validation observations / unique documents | 74 / 37 |
| Covered observations / unique documents | 72 / 36 |
| Covered full pair records | 28 |
| Covered forced one-arm records | 16 |
| Recorded-action agreements | 44 |

`(28 + 16) / (2*28 + 16) = 44/72 = 61.1%`. Every selected covered historical observation carries reward 1.0. This is not 61.1% winner accuracy. It also counts pair records, not independent documents: nine validation IDs were run twice.

The precise reconstruction is saved to `/tmp/policy-resume-audit/historical-metric-reconstruction.json`. It is a source/data audit, not a validated runtime result.

**Minimal correction:** retain `table_accuracy` only as an explicitly named observed-action agreement diagnostic, if needed. Add one **pair/document-level** policy evaluator over a frozen paired cohort. Do not infer winners by comparing with each arm label.

### 2. Mean reward includes the unselected arm (blocking)

`parse_policy_table.rs:356–371` tests only that a table covers a bucket. It adds *every* covered observation's reward; selected code is never compared with `decision_code`. Winner=1/loser=0 therefore averages to0.5 for a complete pair, regardless of the selected arm. In the historical mixed full/forced dataset the function returns61.1%, while selected-observation reward is1.0.

**Minimal correction:** count only measured outcomes for the **selected code**, one per document under one frozen pair/metric, and report a denominator. A covered table selecting a third, unmeasured code is not evaluated as zero reward and is not included as a measured result: report it as `unobserved_decision`. Do not return an unexplained0 for no evaluable cases. The corrected pair evaluator can supply selected reward, native reward, and regret without a second independent implementation.

### 3. Health equates success with winning (high)

`parse_policy_health.rs:148–149`: `ok = obs.reward >= 1.0`, despite `PolicyOutcomeEvidence.ok` already storing parse success. Every successfully parsed loser/reward0 and tie/reward0.5 is counted as failed. In the historical journal all321 outcome records are `ok=true`, but the present aggregate would call only190 “ok”. Those numbers describe wins, not backend reliability. It can trigger a false `backend_ok_rate` alarm for a healthy losing parser. The document gate already uses evidence.ok, so report and triage may disagree.

**Minimal correction:** use outcome.ok for parse-health totals. Observations without outcome evidence are unknown, not proven failures or successes; count missing evidence separately if accepted by the loader, or reject it at the health boundary. If legacy binary-reward fallback is retained, it must be declared and cannot silently apply to paired rewards. Do not change DATA health thresholds to hide the miscount.

### 4. ROUGE-L recurrence reuses a token and can exceed1 (high)

`parse_policy_table.rs:333–342` saves `tmp=dp[j]` but never uses it. On a match it increments `prev_diag`, which is carrying the **current row** value rather than the previous row's diagonal. A single input token can match multiple tokens in the other string.

Independent recurrence examples:

| a | b | Current recurrence | True LCS F1 |
|---|---|---:|---:|
| `a` | `a a` | 4/3 | 2/3 |
| `a a` | `a` | 2/3 | 2/3 |
| `a b` | `b b b` | 6/5 | 2/5 |

The implementation is asymmetric and can exceed1. The existing examples use mostly distinct ordered tokens and miss this.

**Minimal correction:** each row stores `old_up = dp[j]`; on match assign `dp[j] = prev_diag + 1`, otherwise `dp[j] = max(dp[j], dp[j-1])`; then `prev_diag = old_up`. Do not clamp the output: clamping hides the wrong LCS. Current semantics are whitespace-tokenized, ASCII-case-insensitive F1, not a publisher-reference extraction pipeline. No production caller of `rouge_l` was found in the workspace source scan; this is still a latent metric defect and a compile-warning source (`tmp` unused).

### 5. Split is document-disjoint within a run, but not a fixed holdout across cycles (high)

`split_observations` groups all observations by `document_identity`, sorts documents by a64-bit SHA-256 rank, and places each fourth in validation (`parse_policy_table.rs:249–282`). `ObservationSplit::validate` correctly rejects between-side overlap and allows multiple same-document arms on one side. **No direct within-run train/validation leakage was found.**

However, adding a document before an existing rank changes the modulo index and can move old validation documents into training and old training documents into validation. Source-arithmetic example: starting with IDs `d1..d8`, adding `new7` moves `d7,d3` from validation to train and `d6,d2` from train to validation. Comparing Cycle1 and Cycle2 percentages therefore does not compare one stable holdout. Repeated development against those held-out results also makes the validation set unsuitable as an untouched final test.

Historical journal counts:190 pair records/150 unique IDs and321 observations/150 unique IDs. Forty IDs are reruns with different feature vectors; final40 reruns have page_count0/text_quality0. Support floor and reward averages count records rather than independent documents. Re-running the same PDFs did not add40 new samples, and missing classification is not evidence of a0-page/scan PDF. These data details are also covered by sibling `runtime.md`.

**Minimal correction:** freeze the document cohort and split manifest for the experiment. Preserve a separate untouched holdout for any final gain claim. Keep both arms together. Deduplicate/select one explicitly identified valid run per document before train/evaluation; do not silently choose the most favorable reward. Report both record and unique-document counts and feature-provenance failures. A stable hash-modulus split would require a versioned rule change; a manifest preserves the current rule for the frozen cohort without pretending expansion is stable.

### 6. A native baseline must be measured on the same documents (high)

Current DATA uses `page_outline` only when pages>=50 and bookmarks>=1, else `grobid_structural` when quality<0.4, else `xy_cut` (`da-domain/src/fsm_policy.rs:433–443`, `data/policies/registry.yaml`). It is not “alwaysxy_cut”, and the learned b=1 rule differs on short bookmarked articles.

Baseline evaluation must consult an unmodified `NativeFallbackPolicy` loaded from the frozen registry against each validation document's **same measured features**. Candidate evaluation must use actual table lookup/fallback, then native for uncovered buckets. Do not compare a covered subset with an unrelated historical55–60% baseline. Report exact table/native/cohort/metric identities.

The historical fake p0/q0 rerun features make native choose `grobid_structural`, while historical pairs only record `page_outline` and `xy_cut`. The current profile_map maps both GROBID route labels to `arxiv_pdf_dot`, but this is not by itself evidence that every route/config/path is interchangeable. Until explicit frozen execution equivalence is recorded, native reward for a third code is **unobserved**, not fabricated. Correcting feature provenance avoids many such cases.

### 7. Frozen-metric promises are not enforced at the training boundary (high)

A pair carries `metric_digest`, but `pair_to_observations` discards it. `PolicyObservation` has no metric/run identity, and `policy learn` reads all journal observations with only decision-point filtering (`main.rs:1508–1537`). No check prevents mixing binary production observations, old pair metrics, re-scored pairs, or repeated runs. ADR-080 says a retrain can never mix two metric versions; current observation-only learning cannot establish that invariant.

**Minimal correction:** read/freeze pairs as the authority for paired experiments, verify one metric digest/schema before projection, reject or explicitly partition duplicate/conflicting documents, and then derive training observations. Reuse pair evidence rather than synchronizing a second reward journal. Preserve generic single-arm learn as a clearly observational diagnostic that cannot claim policy gain.

### 8. Metric prose and DATA disagree (medium; do not silently change)

ADR-080 D511 says chars/ms is the yield score. Actual `parse_pair_metric.yaml` has `latency_aware:true`, `metric_one:section_count`, `metric_two:body_chars`, so `yield_of` prioritizes **sections/ms**, then chars/ms (`parse_policy_pair.rs:103–122`). A parser recovering4sections/10000chars in1000ms loses to10sections/5000chars under actual DATA, but wins under the stated chars/ms rule.

`PairMetric::from_yaml_str` only checks schema; unknown metric axis strings silently score0. A negative margin is not refused. These are DATA contract failures, not tuning opportunities.

**Minimal correction:** evaluator reports/freezes the actual current rule and digest. Correct ADR language or adopt char-primary only with an explicit recorded metric revision, never as part of an unnoticed evaluator fix. Validate axis names against the supported closed vocabulary and require nonnegative margin. The190 historical full/forced pairs happen not to change winners when swapping throughput axes; that fact does not fix the contract discrepancy.

### 9. Gold-F1 is bounded but is not a type-aware, order-invariant oracle (secondary)

`parse_policy_gold_f1.rs:42–58` ignores `entity_type` and matches labels by substring either direction, greedily in predicted order. Wrong entity types can count as correct. An empty predicted label matches every gold label once. Order example: gold[`alpha`,`alpha beta`], predicted[`alpha beta`,`alpha gamma`] yields1TP, but reversed predicted order yields2TP. Duplicate prevention keeps F1 bounded; it does not make the score invariant or scientifically exact.

`f1_winner` breaks equalF1 on recall then sparsity (`:94–118`), while ADR-080 metricv2 says faster arm. The pure scorer has no latency/applicability/conservation inputs. CLI `run_policy_gold_pair` currently turns absent/failed parse or extraction into empty predictions and separately emits a GoldPairVerdict; it does not feed the trainer or preserve a conservation veto. Do not present this as an interchangeable parse-quality metric or silently use it as the training winner.

**Minimal correction:** keep gold-F1 as a labeled secondary diagnostic for this experiment. Reuse an existing declared reference-matching rule rather than adding another oracle, or explicitly version a type-aware matching rule and tie semantics. Require nonblank labels. If F1 is later adopted as primary, preserve pair applicability/conservation and record the metric revision first. The first current test intentionally supplies type`Model` for all gold types, so type-aware matching would require a deliberate test/contract revision.

## Minimal evaluator output and semantics

One pure evaluator over validated pairs plus per-document candidate/native decisions is enough. Application may accept `FsmPolicyPort`/registry data, not adapter defaults. CLI owns loading paths and frozen-input identity. No route/promote behavior needs changing.

- Unique validation document count and validation pair record count.
- Full-comparison, forced single-arm, tie, both-vetoed, uncovered, and unobserved-decision counts.
- Learned-table coverage / total validation documents, including fallback levels.
- Candidate with native fallback and native-only decision outcomes on the **same evaluable document intersection**, plus changed-decision count.
- Decisive full-pair winner agreement: one vote per document, no forced/tie/veto inflation.
- Selected-arm mean reward and native mean reward; binary regret=`best applicable reward - selected reward`; delta computed on that same intersection.
- Forced-arm success separately; “unobserved code” separately, not assumed a loser or given the other arm's reward.
- If tie rewards are0.5 each, binary-regret0 does not prove good absolute quality. Both-vetoed yields no usable winner; report failures separately. Keep yield/F1/ROUGE and latency outcomes separate when the frozen reward itself is only rank0/0.5/1.
- Whole-cohort fail-open policy results plus covered-only diagnostics; a small successful covered subset is not whole-policy gain.
- Exact frozen split/cohort, input pair digest, metric digest, registry digest, candidate table digest, code revision, valid-feature counts.

With zero evaluable documents or incomplete baseline counterfactuals the result is `not_evaluable`, not `measured_gain`. A small positive paired delta is a measured sample difference, not proof of general superiority. Do not invent a significance threshold or promotion condition during the repair; report sample size and paired deltas for owner review. Locks remain `deployed=false`, `import_eligible=false`, `auto_promote=forbidden`, `graph_write=forbidden`.

## Minimal regression tests

1. **Winner swap:** one covered pair TOC1/GROBID0. TOC-table winner_accuracy1, mean_reward1, regret0; GROBID-table accuracy0, reward0, regret1. Both retain observed-action-agreement1/2 if that diagnostic remains.
2. **Paired native delta:** short bookmarked clean article where native choosesxy_cut and tablepage_outline; TOC wins. Positive candidate-native delta on one document. A second long bookmarked article where native already choosespage_outline adds no changed-decision gain. Both policies evaluated on identical IDs.
3. **Uncovered fallback:** candidate does not cover a validation document; native decision/outcome remains in whole-policy evaluation, coverage drops, no fabricated gain.
4. **Forced/tie/veto:** missingTOC does not inflate decisive winner accuracy; equal rewards are a tie; both-vetoed is a failure/no-winner. Chosen unmeasured`grobid_structural` must be explicit unobserved unless equivalence is proved in the experiment.
5. **Journal identity:** mixedmetric digests, duplicate conflicting features/runs, missing feature provenance, and between-side document overlap fail/partition explicitly. Permuting pair order preserves the report. Two arms remain on the same split side.
6. **Stable-cycle manifest:** adding new training documents must not move a frozen validation identity into train. Keep the present splitter's deterministic/disjoint tests; do not claim they prove fixed holdout stability.
7. **Health outcome vs reward:** two successfully parsed arms with rewards1/0 yieldok2/2; successful tie0.5 isok; failed parse remains failed even if malformed reward1. Missing outcome is unknown/refused by declared contract.
8. **ROUGE oracle:** repeated token cases above; symmetry, range[0,1], and agreement with an independent small two-dimensional LCS oracle across short token sequences. No retry or clamp.
9. **Metric DATA:** unknownaxes/negative margin refused; a deliberate sections/ms-vs-chars/ms conflict pins declared primary axis. Changing primary axis changes metric identity and is explicitly recorded.
10. **Optional gold-secondary contract tests:** reject blank labels; document or fix wrong-type matches; overlapping-label permutation example; pin declared tie-break and preserve applicability/conservation if adopted as primary later.

## Captured external-data identities

Audit read these existing files; it did not mutate them:

- `/root/worktrees/e1-critic/data/agent_memory/policy/observations.jsonl`: SHA-256`8405f1a335dee348055ca7fd3893c40e69f4aaaabcc128e2443da8a18ab55211`
- `/root/worktrees/e1-critic/data/agent_memory/policy/pairs.jsonl`: SHA-256`eadd953559eda10a32d3cecb9f5e202df229b0fca7412f2b40d953c0eb554bc0`
- `/root/worktrees/e1-critic/artifacts/learning/final_table.json`: SHA-256`59e43af91db85d57da2e3839b41a6057aecdeef6ecb30a897ff8483208ab880f`

No assumptions or constants changed. Main worktree historical journal was absent; the old branch data was inspected from the e1-critic worktree. All arithmetic is labeled source/data inspection and must be confirmed by focused Rust regressions before closeout.

## S01 implementation checkpoint

The audit above records the pre-edit defects. Parent-assigned minimal fixes for table_mean_reward, ROUGE-L, health success/missing-outcome rejection, and PairMetric DATA validation are now written in parse_policy_table.rs, parse_policy_health.rs, and parse_policy_pair.rs only. table_accuracy and the larger evaluator/data issues remain intentionally unchanged by this worker. Ten Rust regression functions were added, but this worker ran no cargo/tests/fmt/commit. See `implementation-after.md` for exact changes, limits, impact analysis, and parent-owned verification commands.
