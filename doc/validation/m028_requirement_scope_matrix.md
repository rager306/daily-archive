# M028 Requirement Scope Matrix

Source JSON: `doc/validation/m028_requirement_scope_matrix.json`
Schema: `m028-requirement-scope-matrix.v1`

This fresh-reader twin is metadata-only. It distinguishes M028-covered universal-loader smoke evidence from broad active Scientific KG requirements that remain future/out-of-scope, and it explicitly avoids any no-global-validation leakage.

## Closeout Boundary
M028-8hwqjk proves a bounded universal-loader runtime smoke replay for an accepted expanded mixed-source corpus of 21 URL refs / 20 normalized identities. This S08-updated matrix is metadata-only remediation evidence that separates M028-covered smoke-loader support from broad active Scientific KG requirements that remain out of scope, including R035 validation-batch quota top-up and deterministic replacement materialization.

### In Scope
- Classify R024, R027, and R029 as advanced only by M028 smoke-loader/replay evidence, not globally validated.
- Classify R040 as an active safety constraint preserved and advanced by M028 fail-closed runtime-smoke evidence.
- Classify R036 as already validated and only supported by M028 replay provenance evidence.
- Classify R019, R022, R023, R031, R032, R033, R035, R050, R051, and R052 as future/out-of-scope active requirements for M028.
- Preserve repo-relative evidence paths and explicit forbidden claim language for validation closeout.

### Out of Scope
- Raw article text, PDF bytes, binary payloads, base64 payloads, vectors, secrets, parser/chunker refactors, KG import, LadybugDB writes, graph readiness, trusted fact promotion, DSPy/RLM/MiniMax activation, optimizer behavior, large-batch scaling, crawler behavior, production retrieval evaluation, production scheduler/services, dashboards, or runtime surface changes.

## Requirement Coverage Summary
| Requirement | Status | Classification | Verdict | Action |
|---|---|---|---|---|
| R019 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R022 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R023 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R024 | active | `m028_smoke_loader_evidence_only` | `advanced_by_smoke_loader_not_globally_validated` | remain_active_with_m028_evidence_note |
| R027 | active | `m028_smoke_loader_evidence_only` | `advanced_by_smoke_loader_not_globally_validated` | remain_active_with_m028_evidence_note |
| R029 | active | `m028_smoke_loader_evidence_only` | `advanced_by_smoke_loader_not_globally_validated` | remain_active_with_m028_evidence_note |
| R031 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R032 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R033 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R035 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R036 | validated | `already_validated_requirement_supported_by_replay_provenance` | `supported_not_revalidated_by_m028` | preserve_existing_validated_status |
| R040 | active | `active_safety_constraint_preserved_and_advanced` | `preserved_and_advanced_not_validated` | remain_active_with_m028_safety_evidence_note |
| R050 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R051 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |
| R052 | active | `future_out_of_scope_active_requirement` | `not_advanced_not_validated_by_m028` | remain_active |

## Evidence Sources
- .gsd/REQUIREMENTS.md
- .gsd/milestones/M028-8hwqjk/M028-8hwqjk-ROADMAP.md
- .gsd/milestones/M028-8hwqjk/slices/S06/S06-SUMMARY.md
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/smoke-replay-closeout-summary.json
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/smoke-replay-closeout-events.jsonl
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/smoke-replay-closeout-report.md
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/source-metadata-events.jsonl
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/source-metadata-summary.json
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/pdf-acquisition-events.jsonl
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/pdf-acquisition-summary.json
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/pdf-acquisition-report.md
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/universal-loader-evidence-bundles.jsonl
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/universal-loader-evidence-summary.json
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/universal-loader-evidence-report.md
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/hermes-digest-projection.json
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/hermes-digest-projection-report.md

## Allowed Validation Language
### R019
- M028 leaves R019 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.
### R022
- M028 leaves R022 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.
### R023
- M028 leaves R023 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.
### R024
- M028 advanced R024 only through bounded universal-loader runtime smoke evidence.
- M028 does not globally validate R024 or change the requirement status from active.
- Validation may cite metadata-only S02-S06 replay provenance and safety flags as M028 coverage evidence.
### R027
- M028 advanced R027 only through bounded universal-loader runtime smoke evidence.
- M028 does not globally validate R027 or change the requirement status from active.
- Validation may cite metadata-only S02-S06 replay provenance and safety flags as M028 coverage evidence.
### R029
- M028 advanced R029 only through bounded universal-loader runtime smoke evidence.
- M028 does not globally validate R029 or change the requirement status from active.
- Validation may cite metadata-only S02-S06 replay provenance and safety flags as M028 coverage evidence.
### R031
- M028 leaves R031 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.
### R032
- M028 leaves R032 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.
### R033
- M028 leaves R033 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.
### R035
- M028 leaves R035 as a future/out-of-scope active requirement.
- M028 does not validate or advance R035 because the fixed accepted smoke-loader corpus is not a validation-batch quota/top-up workflow and does not materialize deterministic replacements.
- Validation should not count R035 as a failed M028 deliverable when the M028 boundary excludes accepted-paper quota filling and replacement candidate materialization.
### R036
- M028 supports the already validated R036 provenance requirement with smoke replay provenance evidence.
- M028 preserves R036 evidence by providing repo-relative replay summary, events, report, and per-stage artifact hashes.
### R040
- M028 preserved and advanced R040 as an active safety constraint for the universal-loader runtime smoke boundary.
- M028 safety evidence may be cited without claiming main Scientific KG process activation.
### R050
- M028 leaves R050 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.
### R051
- M028 leaves R051 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.
### R052
- M028 leaves R052 as a future/out-of-scope active requirement.
- Validation should not count broad active Scientific KG requirements as failed M028 deliverables when the M028 boundary excludes them.

## Forbidden Validation Language
### Global
- M028 validates graph readiness
- M028 authorizes KG import
- M028 validates KG import readiness
- M028 validates Scientific KG corpus behavior
- M028 validates import-ready chunks
- M028 promotes trusted facts
- M028 writes to production LadybugDB
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
- M028 proves unattended scaling
- M028 changes parser or chunker behavior
- M028 embeds raw article text, PDFs, binary payloads, base64 payloads, vector payloads, secrets, or production connection details
- M028 fully validates R035
- M028 advances R035 as a deliverable
- M028 changes R035 to validated
- M028 delivers validation-batch quota top-up
- M028 materializes deterministic replacement candidates
### R019
- M028 fully validates R019
- M028 advances R019 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R022
- M028 fully validates R022
- M028 advances R022 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R023
- M028 fully validates R023
- M028 advances R023 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R024
- M028 globally validates R024
- M028 fully validates R024
- M028 validates graph readiness
- M028 authorizes KG import readiness
- M028 validates import-ready chunks
- M028 validates Scientific KG corpus behavior
### R027
- M028 globally validates R027
- M028 fully validates R027
- M028 validates graph readiness
- M028 authorizes KG import readiness
- M028 validates import-ready chunks
- M028 validates Scientific KG corpus behavior
### R029
- M028 globally validates R029
- M028 fully validates R029
- M028 validates graph readiness
- M028 authorizes KG import readiness
- M028 validates import-ready chunks
- M028 validates Scientific KG corpus behavior
### R031
- M028 fully validates R031
- M028 advances R031 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R032
- M028 fully validates R032
- M028 advances R032 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R033
- M028 fully validates R033
- M028 advances R033 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R035
- M028 fully validates R035
- M028 advances R035 as a deliverable
- M028 changes R035 to validated
- M028 delivers validation-batch quota top-up
- M028 materializes deterministic replacement candidates
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R036
- M028 newly validates R036 globally
- M028 closes all future provenance validation obligations
- M028 authorizes production LadybugDB writes
### R040
- M028 fully validates R040
- M028 enables new infrastructure in the main Scientific KG process
- M028 authorizes production writes, KG import, graph readiness, or model/helper activation
### R050
- M028 fully validates R050
- M028 advances R050 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R051
- M028 fully validates R051
- M028 advances R051 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior
### R052
- M028 fully validates R052
- M028 advances R052 as a deliverable
- M028 validates Scientific KG corpus behavior
- M028 validates graph readiness
- M028 authorizes KG import
- M028 proves unattended scaling
- M028 activates DSPy, RLM, MiniMax, or optimizer behavior

## Requirement Notes
### R019
- Rationale: hybrid retrieval with vector, graph, fusion score metadata, and EvidencePath references remains future Scientific KG work
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.
### R022
- Rationale: RLM document navigation and workflow-in-code prototypes remain future bounded helper work
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.
### R023
- Rationale: RLM graph traversal benchmarks against deterministic baselines remain future work
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.
### R024
- Rationale: M028 supplies bounded smoke-loader evidence for an accepted 21-ref corpus, but staged 10-document, 20-document, one-week Scientific KG graph-quality validation remains unproven
- Evidence: S06 replay closeout status is pass for the exact 21 URL refs / 20 normalized identities smoke corpus.
- Evidence: Replay artifacts include source metadata, PDF acquisition diagnostics, universal-loader evidence bundles, and Hermes digest projection derived from evidence bundles only.
- Evidence: Safety flags and unsafe counters keep parser, chunker, KG/import, graph readiness, model/helper, crawler, large-batch, and production-write behavior fail-closed.
- Remaining work: A future milestone must supply canonical validation evidence before status can change from active.
- Remaining work: Graph import, trusted facts, production writes, chunk/import readiness, broader corpus validation, and unattended scaling require separate explicit gates.
### R027
- Rationale: M028 advances source/acquisition/evidence provenance around smoke-loader handoff fields, but does not provide graph-readiness quality acceptance for conversion, chunk semantics, or downstream KG use
- Evidence: S06 replay closeout status is pass for the exact 21 URL refs / 20 normalized identities smoke corpus.
- Evidence: Replay artifacts include source metadata, PDF acquisition diagnostics, universal-loader evidence bundles, and Hermes digest projection derived from evidence bundles only.
- Evidence: Safety flags and unsafe counters keep parser, chunker, KG/import, graph readiness, model/helper, crawler, large-batch, and production-write behavior fail-closed.
- Remaining work: A future milestone must supply canonical validation evidence before status can change from active.
- Remaining work: Graph import, trusted facts, production writes, chunk/import readiness, broader corpus validation, and unattended scaling require separate explicit gates.
### R029
- Rationale: M028 emits universal-loader evidence bundles with stable provenance, but does not produce import-ready typed chunk packages or independent semantic review evidence
- Evidence: S06 replay closeout status is pass for the exact 21 URL refs / 20 normalized identities smoke corpus.
- Evidence: Replay artifacts include source metadata, PDF acquisition diagnostics, universal-loader evidence bundles, and Hermes digest projection derived from evidence bundles only.
- Evidence: Safety flags and unsafe counters keep parser, chunker, KG/import, graph readiness, model/helper, crawler, large-batch, and production-write behavior fail-closed.
- Remaining work: A future milestone must supply canonical validation evidence before status can change from active.
- Remaining work: Graph import, trusted facts, production writes, chunk/import readiness, broader corpus validation, and unattended scaling require separate explicit gates.
### R031
- Rationale: 30-paper deviation scans and broader chunking/import-readiness conclusions remain future validation work
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.
### R032
- Rationale: automated +10-paper iterative validation loops toward 100 papers remain future operability work
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.
### R033
- Rationale: deterministic resumable +10-paper validation batch CLI remains future operability work
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.
### R035
- Rationale: M028's fixed accepted smoke-loader corpus is not a validation-batch +10 quota/top-up workflow and does not materialize deterministic replacements.
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: M028 uses a fixed accepted smoke-loader corpus and does not execute a validation-batch +10 quota/top-up workflow.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers automatic accepted-paper replacement acquisition, deterministic replacement materialization, parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep R035 active until a future validation-batch milestone supplies direct canonical evidence for accepted-paper quota top-up and deterministic replacement materialization.
- Remaining work: Do not expand M028 validation beyond the fixed universal-loader runtime smoke corpus boundary.
### R036
- Rationale: R036 is already validated and M028 only contributes additional replay provenance support for the smoke-loader closeout.
- Evidence: R036 is already validated in .gsd/REQUIREMENTS.md before this S07 remediation task.
- Evidence: S06 replay provenance records command, cwd, git commit, input hashes, output hashes, exit code/status, milestone/slice context, diagnostics, and safety state for S02-S05 replay stages.
- Evidence: M028 support for R036 is limited to replay/audit provenance evidence for this smoke closeout; it does not create a new global validation claim for every future artifact workflow.
- Remaining work: Future generated artifact classes must continue to record command, inputs, outputs, hashes, exit status, cwd, git commit, and milestone/batch context.
### R040
- Rationale: M028 respects and advances the safety-wrap principle, but R040 remains an active project-wide constraint rather than a closed milestone deliverable.
- Evidence: M028 preserved local-first safety by restricting S02-S06 to metadata-only replay surfaces and validate-only checks.
- Evidence: S06 safety flags show network/model/crawler/parser/chunker/graph/write/KG readiness behavior false and unsafe counters zero.
- Evidence: M028 advances R040 as evidence that new runtime loader boundaries were smoke-tested and safety-wrapped before any broader Scientific KG process activation.
- Remaining work: Keep R040 active as a project-wide constraint for future infrastructure work.
- Remaining work: Future infrastructure milestones still need research/probe artifacts, redaction boundaries, failure-mode analysis, and explicit go/no-go decisions.
### R050
- Rationale: article-structure artifact detection and candidate KG scaffold links remain future pre-KG CLI work
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.
### R051
- Rationale: MiniMax helper integration for article artifact detection remains future bounded structured-helper work and was not invoked by M028
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.
### R052
- Rationale: DSPy prompt optimization remains gated until benchmark fixtures, metrics, and baseline outputs exist and was not invoked by M028
- Evidence: M028 scope is limited to the expanded mixed-source universal-loader runtime smoke path over 21 URL refs / 20 normalized identities.
- Evidence: S06 closeout evidence is metadata-only and explicitly defers parser/chunker refactors, graph import, LadybugDB writes, model helpers, large-batch ingestion, crawler behavior, production retrieval evaluation, and positive KG readiness.
- Remaining work: Keep this requirement active until a future milestone targets it directly with canonical validation evidence.
- Remaining work: Do not expand M028 validation beyond the universal-loader runtime smoke boundary.

## Validation Recommendation
Accept M028 evidence only for bounded smoke-loader advancement/support and R040 safety preservation while leaving broad active Scientific KG requirements active and out of scope.
M028-covered does not mean globally validated. Future/out-of-scope active requirements are intentionally excluded from M028 success criteria and must not be marked failed merely because M028 did not address them.
- R024, R027, and R029 were advanced only through M028 metadata-only smoke-loader replay evidence.
- R040 was preserved and advanced as an active safety constraint.
- R036 was already validated and is only supported by M028 replay provenance.
- R019, R022, R023, R031, R032, R033, R035, R050, R051, and R052 remain active future/out-of-scope requirements.

## Safety Flags
- `metadata_only`: `true`
- `network_fetch_attempted`: `false`
- `runtime_surface_changed`: `false`
- `parser_or_chunker_changed`: `false`
- `kg_import_or_readiness_claimed`: `false`
- `graph_validation_claimed`: `false`
- `import_ready_chunks_claimed`: `false`
- `trusted_fact_promotion_claimed`: `false`
- `scientific_kg_corpus_validation_claimed`: `false`
- `dspy_rlm_minimax_activation_claimed`: `false`
- `unattended_scaling_claimed`: `false`
- `raw_payloads_embedded`: `false`
- `binary_payloads_embedded`: `false`
- `base64_payloads_embedded`: `false`
- `vector_payloads_embedded`: `false`
- `secrets_embedded`: `false`
- `production_ladybugdb_writes_claimed`: `false`
- `production_write_attempted`: `false`

## Failure Modes
- Filesystem dependency only: missing or malformed JSON/markdown/evidence paths should bubble as verifier diagnostics or nonzero shell exit.
- No API, network, database, model, crawler, graph, or runtime service dependency is introduced by this task.

## Load Profile
- Expected load is one local JSON matrix, one markdown twin, and fifteen requirement rows.
- At 10x rows, local filesystem reads and deterministic string/JSON scans saturate first and remain linear; no pooling, pagination, rate limiting, or caching is needed.

## Negative Tests
- Current T01 verification covers malformed JSON via json.tool and executable verifier checks for missing rows, duplicate rows, unsafe true booleans, unsafe claim leakage, stale markdown, invalid evidence paths, and false global validation wording.
- Focused tests include R035 future/out-of-scope false-validation and unsafe quota/replacement positive-claim scenarios.

## Observability Impact
Adds human and machine-readable metadata-only validation surfaces that make requirement coverage ambiguity, forbidden global-validation claims, R035 quota/replacement scope boundaries, safety flags, evidence paths, and future verifier diagnostics inspectable without changing runtime behavior.

## Verifier Expectations
- JSON parse command: `uv run python -m json.tool doc/validation/m028_requirement_scope_matrix.json`
- Markdown nonempty command: `test -s doc/validation/m028_requirement_scope_matrix.md`
- Future validate-only diagnostics should use JSON paths such as `$.requirements[<index>].<field>` and `$.safety_flags.<flag>`.
- JSON parses as object and contains every required_requirement_ids row exactly once.
- Markdown references Source JSON: `doc/validation/m028_requirement_scope_matrix.json`.
- Markdown requirement table contains every required requirement row.
- Safety flags remain metadata-only/fail-closed and no raw/binary/vector/secret payload fields are embedded.
- Allowed claims do not imply global validation for future/out-of-scope active requirements.
- Forbidden claim list keeps parser/chunker, KG/import, LadybugDB, DSPy, RLM, MiniMax, scaling, readiness, raw-payload, and production-write claims blocked.
- Validate-only success reports 15 requirement rows checked.
- `M028_MATRIX_JSON_MALFORMED`
- `M028_MATRIX_REQUIRED_ROW_MISSING`
- `M028_MATRIX_REQUIRED_ROW_DUPLICATE`
- `M028_MATRIX_UNSAFE_FLAG_TRUE`
- `M028_MATRIX_UNSAFE_CLAIM_LEAKED`
- `M028_MATRIX_MARKDOWN_STALE`
- `M028_MATRIX_EVIDENCE_PATH_INVALID`
