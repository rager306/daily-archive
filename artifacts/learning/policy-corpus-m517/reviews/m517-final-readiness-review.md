# M517 final readiness review

2026-10-13. Read-only review of candidate-only capture/evaluation, corrected frozen inputs, scope, provenance, parent-owned test receipts and the earlier independent review. No source/config edits, cargo, parser runs, KG writes or provider changes by this reviewer. Only this report was written.

## Verdict

**No remaining source, manifest or scoring blocker found after parent corrections.** The corrected manifest is now complete and source-family disjoint. One provenance ambiguity needs correction before final result freeze/closeout. No additional defect was found in per-document denominators or selected-policy aggregation. The two manifest defects found during this review were corrected before any evaluated cycle and remain documented below.

### 1. Resolved blocker: two replacement rows initially lacked capture_dir

Manifest SHA `ee7cfe2df0660d29cd55d80ff7a2b3ba7308bf5583448e19976ee4f79b29ab87` has320 distinct source-family IDs and320 distinct PDF hashes, but the replacement rows lack `capture_dir`:

- `manifest.json:2899–2905`: `2506.18403`, SHA `a2d66c3f224c4340156610fb95fa117ccf5cf09002f3d4ed856b85e7c4fa9ab7`.
- `manifest.json:2907–2913`: `2604.14228`, SHA `a4c3c92070557d693125850ab40d8249dbb3967988c919e95a6f46caa4ab62b9`.

`crates/da-cli/examples/policy_corpus/evaluation.rs:41–50` requires the field with no default. `run():792–794` deserializes the entire manifest, not only the selected prefix. Thus **all three cycles will fail before scoring**, even though both additions appear near the end of the training list. Orchestration that reads `doc["capture_dir"]` can also fail at full-corpus capture.

Parent corrected both directory bindings. Final manifest SHA is `dc2c75a045bbba8e9d90b4fc90e3b2fa5bdaf07276be573808bac395164936a1`. I verified all320 document fieldsets are complete, all capture directories equal their expected authoritative SHA path, and the only change from `ee7cfe…` is these directory fields. Source order, membership, partitions and scoring did not change. Both provenance files now bind this final manifest SHA. Already captured documents retain unchanged bindings/recipe.

### 2. Important: evaluator provenance contains contradictory executable/source identities

`evaluator-provenance.json:3–7` still presents the old `executable_sha256=b34d87…` and the old `sources` map. Lines16–19 add the new source hashes and `evaluator_executable_sha256=7730cb…` instead of replacing the original fields. The actual executable currently hashes to `7730cb76be2b8213dee833d4c03853b285217d0fe89625f0ec980280ae75c875`. Current source hashes match the new fields, not the old map.

This is not a scoring defect, but a final recipe/comparability artifact must not have two answers for executable/source identity. Consumers of the original schema fields will attribute the new campaign to the old executable. Action: keep one canonical current `executable_sha256` and `sources` mapping. Preserve any previous identity under an explicitly labeled preflight/history record. The manifest hash now agrees with the final corrected manifest. Capture provenance agrees with the repaired capture executable and latest sources.

## Resolved blocker: source-family collision and cross-partition family

Original manifest SHA `a2a0459e5d4478924b0f72c1fa4150ab395922b60cdec9db5892470e377cf684` contained320 unique PDF hashes but318 unique IDs. `trench-real-analysis` appeared at original lines685,1027,2404: full book in validation and two supplements in training. The evaluator's duplicate-ID guard would reject it; removing that guard or only renaming supplements would leave a source-family split concern.

Parent preserved the original as `preflight-manifest.json`, excluded extra source-family PDFs, selected two replacement primary-source IDs, and refroze before any scored cycle. The current manifest has320 IDs/320 hashes,240train/80validation, is SHA-sorted and applies validation to zero-based indices divisible by4. This changes52 validation members compared with the invalid preflight partition; it is acceptable only because no valid cycles ran and preflight results are explicitly excluded. The corrected fixed validation set, not the old pilot, must be used for every cycle.

## Independent checks

- Read and rehashed every currently selected PDF: **320/320 source SHA and byte counts match**. No source exceeds6,000,000 bytes.
- No corrected source ID overlaps the150 historical IDs. No corrected SHA overlaps151 historical source PDFs located under the main repository. Historical copied journals/table match all frozen hashes and byte counts.
- Final corrected manifest is source-family unique and complete; finding1 is resolved. All320 capture directories are unique, SHA-bound and do not point into `preflight-captures/`.
- Experiment, native registry and legacy metric copies exactly match their DATA sources. Constants remain min_support4arm observations,5-token grams and quality tie margin0.02. No metric/threshold change by reviewer.
- **15 repaired authoritative captures** were inspected after parent restarted capture:13 `success/success`,2 `not_applicable/success`. Measured bookmark counts are positive13/zero2. Every inspected source/exporter/furniture/observations/reference/article hash, paper identity, page/bookmark/source relationship and evidence section/body-character/latency count matches. Counts describe only this moving inspection snapshot, not all320. No inference from arXiv/category labels.
- All132 invalid preflight captures remain in the separate retained directory; none is referenced by the current manifest. No score or training input was taken from them.

## Scoring and learning assessment

- Each validation PDF produces one row. Native and candidate consult the same measured features, capture, reference and pair score (`evaluate():673–695`). Selecting failed/NA output stays failed/NA with reward0; no fabricated reroute.
- Selected reward, regret, latency and operational-success rate use all assigned documents. Quality means use successful selected outputs only and expose n (`summary():711–740`). Decisive full-pair accuracy excludes forced/tie/failure cases and has its own n. Operational success comes from successful outcome evidence, not winner reward.
- Failure cases are scored as0, successful ties as0.5, successful forced choices as1. Both-failed regret0 remains an oracle-relative statement, not operational success (`score():494–585`).
- Token and5-gram scores use multiset intersection and explicit units. Zero reference/candidate denominators are not fabricated as score1. Reference suitability is checked before scoring. The5-gram metric measures local order, not global reading order.
- Training excludes only typed NotApplicable observations and retains measured failures (`run():843–871`). Forced zero-outline cases therefore contribute one GROBID observation, while full pairs contribute two. The feature bucket includes measured bookmark presence; exact paired cells do not mix empty-TOC records into the TOC arm mean. Minimum support remains total arm observations, not equal support per arm or confidence in four documents; scope already states this correctly.
- Existing learned-table lookup can drop bookmark presence at its last fallback level. A sparse positive-bookmark exact cell can therefore inherit a zero-bookmark GROBID cell built from forced choices. This is the **actual existing candidate policy**, not validation leakage or an evaluator error; the matched cell key/level/support and selected reward remain visible. Do not interpret learned coverage as full-pair evidence in every positive-bookmark bucket, or assume all exact-uncovered buckets answer native. No fallback or support change is recommended mid-campaign.
- Validation data never enters training observations; training prefixes stay60/120/240 and validation membership comes only from the frozen manifest. Macro means are one contribution per document, not arm-record means. Per-document validation rows are retained for parent-owned cross-cycle regression analysis.

## Earlier review and metadata repair

The TOC-schema bug remains fixed: evaluator loads the common paper/hash/section-text view rather than requiring GROBID `Section.level`. Parent test receipt reports10 evaluator tests passed, including genuine `build_pdf_toc_stage` loading and tamper rejection. This reviewer did not rerun tests.

Repaired capture writes Python/PyMuPDF versions into separate `toc-versions.json` (`policy_corpus_capture.rs:168–176`) and reads that file at195, never stdout. Native diagnostics cannot corrupt the metadata object. Parent repair test receipt reports5capture tests passed, including this boundary. `stdout/stderr` files are retained for successful exporter processes. On nonzero exporter exit, the current early ensure at188–192 runs before diagnostic writes193–194: stderr is embedded in the error, but standalone stdout/stderr files are not written. This is a bounded failure-diagnostics limitation, not a successful-capture/scoring blocker and not evidence to silently turn an export failure into NotApplicable.

Report metric identity now embeds evaluator-source hash (`evaluation.rs:926–928`); binary provenance remains the broader recipe identity. Scoring cycles must use the same frozen binary/captures/registry/config and produce separate report paths. No cycle output existed when this review inspected source; final campaign regression/completion claims still require actual parent-owned runs.

Scope correctly labels section-text versus full-PDF reference, adaptive diagnostics rather than untouched test, native-DATA counterfactual rather than literal production routing, and one noisy wall-clock sample rather than controlled speedup. No KG-readiness or body-only-fidelity claim is supported.

## Review bindings and limits

Sources were formatted by parent while review ran. Final source SHA-256 values inspected:

- capture: `f0f90652f7b6c89910e22616681e171962a412aaad308e72b7d1ae6fd793bac9`
- shared carriers: `a05d914bb51dab3d2695df9eff3c5aef5c5651f03000f81ccacd0b40070cf64b`
- evaluation: `e863d68dc480f32a1266c51009defef01f2db39e77f817b767d1d304314dc75e`
- evaluator entry: `d97f950f4a5267c445cf7430485e6301ad89af0886457ee3720a3552720fc458`

Actual binaries: capture `f2cd924d0345a971449fab4dca37321188fa25a3385b05c073b84abb5a3dfebc`; evaluator `7730cb76be2b8213dee833d4c03853b285217d0fe89625f0ec980280ae75c875`.

GitNexus repository bound to daily-archive; index reports2026-10-13T10:00:26Z at718be28b5. MCP query/context disconnected and CLI query failed to launch, so graph completeness is unresolved. Direct named source/known-call inspection was used, no source edit depended on incomplete graph results. No compilation/test execution claim by reviewer. Constants and production files unchanged.
