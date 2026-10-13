# M517 paired-policy cycles

## Result

All three candidate-only cycles completed: training prefixes of 60, 120 and 240 PDFs, evaluated on the same 80 validation PDFs. Decisions and selected outputs did not change between cycles. Support and training gaps changed; no new independent validation documents were added.

**An overall parse-quality gain is not established.** Seven metric-specific tables choose the same outputs. They raise mean token F1 from 0.798908 to 0.871408 but lower mean local ordered-5gram F1 from 0.712665 to 0.688143. Ordered-5gram reward improves because the policy wins on more documents, not because its average raw score improves. There are 36 gains and 31 larger losses; 20 of those ordering losses coexist with token-F1 gains. The `sections` candidate makes the same validation choices as native and has no gain.

No metric was adopted. Production routing, import, deployment and graph writes remain unchanged and locked. This is evidence about a bounded candidate-policy experiment, not GROBID retirement, publisher fidelity, scientific KG readiness or production speedup.

## Corpus and freeze

The final manifest has 320 distinct PDF SHA-256 hashes and 320 source-directory family IDs: 240 training and 80 validation. Training prefixes grow without changing the validation partition, captures, binaries or eight declared metrics. Historical IDs and hashes are excluded. Selection is SHA-sorted under a 6,000,000-byte cap, with one PDF per source-directory family; it is not a representative random sample. Directory-ID separation does not prove publication-level independence for related works under different IDs. Validation is an adaptive diagnostic set, not an untouched final test.

Final manifest SHA: `dc2c75a045bbba8e9d90b4fc90e3b2fa5bdaf07276be573808bac395164936a1`.
Experiment SHA: `b7dc7fafd3832d95e7275335fb1f4e2dc20a3f1dc1ed21bc8e06ff047755b31e`.

All 320 GROBID structural arms succeeded. TOC succeeded on 274 PDFs and was NotApplicable on 46 with empty outlines; no failed arm was observed. The fixed validation has 68 successful pairs and 12 empty-TOC forced GROBID choices. Missing/failure/tamper behavior is covered by tests, not inferred from these successful live captures.

All 132 preflight captures are preserved and excluded. Native diagnostic stdout had corrupted the old metadata transport; the corrected runner reads a separate `toc-versions.json`. The original family-colliding manifest and earlier provenance remain as preflight evidence. Corrections were frozen before any scored cycle. The repaired smoke document is part of the final cohort and was captured under the same authoritative runner; it is not a reused invalid pilot.

## Selected-policy reward

Every value below is the same in cycles 60, 120 and 240. Reward is a metric-specific rank: winner 1, loser/failed/unavailable 0, successful tie 0.5. It is not the raw metric value. Reward, regret, success and latency use all 80 assigned validation documents. Decisive accuracy excludes the 12 forced successes and any ties.

| Metric | Native reward | Candidate reward | Candidate decisive correct/n | Improved/regressed vs native |
| --- | ---: | ---: | ---: | ---: |
| `body_chars` | 0.16250 | 1.00000 | 68/68 | 67/0 |
| `chars_per_ms` | 0.16250 | 1.00000 | 68/68 | 67/0 |
| `legacy_yield` | 0.20000 | 0.96250 | 65/68 | 64/3 |
| `sections_per_ms` | 0.20000 | 0.96250 | 65/68 | 64/3 |
| `token_f1` | 0.30000 | 0.86250 | 57/68 | 56/11 |
| `ordered_5gram_f1` | 0.55000 | 0.61250 | 37/68 | 36/31 |
| `quality_then_latency` | 0.55000 | 0.61250 | 37/68 | 36/31 |
| `sections` | 0.79375 | 0.79375 | 50/65 | 0/0 |

All native and candidate selections succeed 80/80. Learned coverage is 79/80: 67 bookmarked short PDFs plus 12 empty-TOC PDFs. The 587-page book uses native fallback. All learned matches are exact; domain/bookmark fallback levels are not exercised by validation. The `sections` table selects 79 GROBID and one TOC; every other table selects 12 GROBID and 68 TOC. These are two selected policies, not eight independent gains.

Forced-selected reward is 1 for both policies on 12 documents without a full pair. For ordered 5grams, 49/80 candidate reward and 44/80 native reward become 37/68 and 32/68 decisive comparisons. The forced choices raise both headline rates but do not create their paired delta.

## Raw diagnostics and regressions

Candidate text concatenates `sections.text`; the independent reference is full-PDF `pdftotext`. Successful-selected means all have n=80 here.

| Diagnostic | Native / `sections` | Every non-section candidate |
| --- | ---: | ---: |
| Mean token F1 | 0.7989084818 | 0.8714077329 |
| Mean local ordered-5gram F1 | 0.7126654228 | 0.6881427908 |
| Mean section-text characters | 61,570.7875 | 93,231.2 |
| Mean selected captured latency, ms | 6,018.05 | 2,398.1 |

The 31 ordering losses outweigh the 36 gains in raw mean score. The largest loss is `2505.03906`: 5gram F1 falls from 0.854438 to 0.239889 and token F1 from 0.881558 to 0.479508. On `2410.13610`, token F1 rises from 0.889791 to 0.946353 while 5gram F1 falls from 0.838424 to 0.431974. Every regression's PDF, reference, arm and matched-cell hashes are retained in the cycle reports and `policy-corpus-cycles.json`; none is removed from a denominator.

Exact repeated TOC section text is present in 24 of the 68 successful validation TOC outputs. Concatenation adds 771,926 duplicate characters. On `2505.03906`, multiple headings on the same page repeat its entire text: 31 sections contain 10 distinct payloads, with 169,870 concatenated characters versus 60,110 after exact-payload deduplication. This is an observed cause of inflated volume, not a replacement segmentation rule. No deduplication or parser repair was applied during scoring. The frozen repetition diagnostic is `validation-toc-exact-repetition.json`.

Token and ordered-5gram winners disagree on 20/68 decisive pairs. Body-character and ordered-5gram winners disagree on 31/68. These metrics can disagree even when coarse-cell majority training picks identical actions. No validation pair lies within the 0.02 quality margin, so the quality-then-latency tiebreak is not exercised there. Its training winners can differ. Legacy yield retains integer sections/ms primary plus integer chars/ms secondary; it was not silently converted to char-primary throughput.

## Support and cycle comparison

Minimum support remains four arm observations, not four distinct PDFs or four observations per arm. Empty-TOC successes can support a GROBID cell without a counterfactual TOC score. All measured domains are empty and quality is in the high bucket; low-quality scans and nonempty-domain cells are not evaluated.

| Exact cell | Distinct train PDFs: 60 / 120 / 240 | Arm observations: 60 / 120 / 240 | Validation matches |
| --- | ---: | ---: | ---: |
| `p=1_49;b=0;q=ge700;d=_` | 7 / 13 / 33 | 7 / 13 / 33 | 12 |
| `p=1_49;b=1;q=ge700;d=_` | 50 / 100 / 195 | 100 / 200 / 390 | 67 |
| `p=50_199;b=1;q=ge700;d=_` | 3 / 7 / 11 | 6 / 14 / 22 | 0 |

The final training set also has one 50–199-page empty-TOC document; its support is below the table threshold. Training observations total 113 / 227 / 446. Each cycle retains three candidate cells; none is deployed. Both transitions have zero changed validation executions, outcomes or raw scores. More support is not another validation sample or proof that these coarse features suffice.

| Selected-reward train-minus-validation gap | 60 | 120 | 240 |
| --- | ---: | ---: | ---: |
| `token_f1` | 0.054167 | 0.020833 | 0.033333 |
| `ordered_5gram_f1` | 0.187500 | 0.070833 | 0.062500 |
| `quality_then_latency` | 0.187500 | 0.079167 | 0.070833 |
| `sections` | 0.031250 | 0.014583 | -0.014583 |

All eight gaps are retained in the JSON report. A lower gap without changed validation outcomes is not measured improvement in generalization.

## Scope limits

- The baseline is actual native DATA decisions under a frozen execution map: `xy_cut` and `grobid_structural` map to captured Java structural GROBID, `page_outline` to TOC. Literal current production `parse-pdf` still directly invokes GROBID after its grammar consult. No separate XY-Cut arm ran.
- Full-PDF reference versus section-only candidate measures limited text retention, not body-only fidelity. Header, abstract, citation and table fields outside sections are not scored. ASCII alphabetic token normalization excludes numbers, non-ASCII text and math. Token overlap ignores order; local 5gram multiset overlap does not prove global reading order.
- One captured wall-clock sample includes arm setup/export/probe/parse/write, on a shared host. Classification/reference work is excluded. Replaying those captures is not a new timing sample. No controlled production speedup or tenfold capture/evaluator capacity is claimed.
- These are trusted local offline tools, not an output-path sandbox. Capture refuses an existing output directory and production `data/`/pipeline-cache roots. Evaluation writes to caller-supplied report/table paths and can overwrite those paths. The recorded campaign and replay used separate isolated output directories; the final hashes and production baseline verify those invocations only. Managed or multi-tenant use needs a separate output-root/no-overwrite contract.
- No metric, threshold, support floor, 5gram size, margin or routing default changed during the three cycles. A post-measurement clippy repair collapses one domain nested `if` without changing lookup behavior; the original source and runner hashes are retained. It does not relabel the frozen measurement as a new build.
- Final architecture verification also requires the ADR-080 offline capture constructor in the closed test inventory. That exception requires the exact explicit structural profile. Production corpus callers still require `corpus_grobid_parser()`. The inventory changes from four to five sites; no measured source or runner changes.
- An inherited endpoint timeout test used a real 30ms deadline, then polled after failure for a request that might never have reached the stub. It now pauses the Tokio clock until the stub receives the request, then advances 31ms. The 30ms client and 250ms server durations are unchanged. This changes test determinism, not production behavior or measured arms.

## Verification and disposition

Scoped current-source tests passed: application 565, CLI 421, capture 5, evaluator 10, opt-in 5 and retired-action 1. Library/tests and feature-enabled example clippy passed. Runtime UAT `1db6bc9a-e358-4a62-92f8-8b72b7e3236d` ran 87 policy checks. Artifact UAT `8cd76ab8-466f-4710-a268-13e64b0f9240` verified all 320 source/capture/reference/arm bindings, independently recomputed validation text scores and selected-reward aggregates, and checked unchanged historical hashes and production file paths/sizes/timestamps. Fresh artifact UAT `0aba1b91-7596-4fea-a54a-c3753d2df8f6` repeated those checks and verified the current-source seal and post-measurement repair records. No UAT timed out, aborted or received a signal.

Offline replay of cycle 240 with the frozen evaluator reproduced every report field after normalizing only table output paths. Captures, scores, table hashes, support and all regressions match. Artifact UAT and offline replay are retained-artifact evidence, not another live parse or timing sample.

The full architecture gate passed all 23 steps with 0 timed out and exit 0. Its actual log and current-source receipt are under `verification/`. Final `cargo fmt --all` and `cargo fmt --all --check` exited 0 without changing Rust source bytes. Closeout artifact UAT `6d0465d8-4faa-4153-a12f-d3b71c1dd866` passed the preserved independent arithmetic/hash oracle plus full architecture, current-source, review, replay, process and service/disk assertions in 20,765 ms, without timeout, abort or signal. Source-bound GSD closeout remains separate from these checks. GSD delivery occurred outside canonical Task Attempts and requires the documented cancellation/waiver path, not fabricated task completion. Candidate-only locks remain `deployed=false`, `import_eligible=false`, `auto_promote=forbidden`, `graph_write=forbidden`.

## Evidence

- Detailed report and full regressions: `doc/review/policy-corpus-cycles.json`.
- Frozen manifest, eight tables per cycle, raw outputs, provenance, corrections and receipts: `artifacts/learning/policy-corpus-m517/`.
- Independent result reconstruction and geometry limitations: `artifacts/learning/policy-corpus-m517/reviews/`.

Next evidence, before any proposed adoption: fix TOC page-boundary repetition and reading order under a new recipe; evaluate declared metrics with magnitude-sensitive diagnostics and independent suitable references; cover missing feature/availability cases; use a separately frozen untouched test. These are follow-ups, not changes hidden in the completed cycles.
