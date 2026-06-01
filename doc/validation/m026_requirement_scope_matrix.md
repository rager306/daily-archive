# M026 Requirement Scope Matrix

## Closeout Boundary
M026 is a loader research, Hermes digest contract, universal loader contract, and real-source validation milestone. It supplies conservative evidence and boundary language only; it does not implement the loader, batch selection, KG import/readiness, graph validation, corpus validation, or AI helper activation.

### In Scope
- arXiv source-capability research and sample validation evidence.
- Hermes digest contract boundaries and metadata-only redaction requirements.
- Universal loader contract boundaries, compatibility notes, failure modes, and future implementation gaps.
- Requirement-scope reconciliation for all active requirements plus touched validated/context requirements.

### Out of Scope
- loader implementation
- batch selection implementation
- KG import or import readiness
- graph validation or graph readiness acceptance
- import-ready chunks
- Scientific KG corpus validation
- DSPy, RLM, MiniMax, or optimizer activation
- raw article text, PDF bytes, binary payloads, vector payloads, secrets, or production connection details
- production LadybugDB writes

## Requirement Coverage Summary
| Requirement | Status | Classification | Verdict | Action |
|---|---|---|---|---|
| R001 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R002 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R003 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R004 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R005 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R006 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R007 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R008 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R009 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R010 | validated | `existing_hermes_daily_archive_context` | `existing_coverage_context_not_revalidated` | preserve_existing_validated_status |
| R014 | validated | `existing_validated_compatibility_context` | `existing_coverage_supported_not_revalidated` | preserve_existing_validated_status |
| R019 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R022 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R023 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R024 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R027 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R029 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R031 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R032 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R033 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R035 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R036 | active | `adjacent_evidence_not_full_requirement` | `advanced_not_validated` | remain_active |
| R040 | active | `in_scope_constraint_followed` | `satisfied_for_m026_not_globally_validated` | treat_as_followed_constraint_keep_active |
| R050 | active | `out_of_scope_future_consumer` | `not_implemented_not_validated` | remain_active |
| R051 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R052 | active | `out_of_scope_active_requirement` | `not_advanced_not_validated` | remain_active |
| R030 | validated | `existing_validated_supporting_context` | `existing_coverage_supported_not_revalidated` | preserve_existing_validated_status |

## Evidence Sources
- `.gsd/REQUIREMENTS.md`
- `.gsd/milestones/M026-3rvvgp/M026-3rvvgp-ROADMAP.md`
- `.gsd/milestones/M026-3rvvgp/slices/S05/S05-RESEARCH.md`
- `.gsd/reports/article-artifact-loader-research.md`
- `.gsd/reports/article-artifact-loader-digest-contract.md`
- `.gsd/reports/article-artifact-universal-loader-contract.md`
- `.gsd/reports/article-artifact-loader-validation.md`

## Allowed Validation Language
### R001
- M026 cites R001 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R002
- M026 cites R002 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R003
- M026 cites R003 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R004
- M026 cites R004 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R005
- M026 cites R005 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R006
- M026 cites R006 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R007
- M026 cites R007 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R008
- M026 cites R008 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R009
- M026 cites R009 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R010
- M026 cites R010 only as existing Hermes/daily-archive compatibility or historical coverage context.
### R014
- M026 contracts remain compatible with existing local markdown/plain-text ingestion results.
- M026 cites R014 as existing validated compatibility context only.
### R019
- M026 leaves R019 active and out of scope for validation closeout.
- Hybrid retrieval evidence contexts are outside M026 loader-research scope.
### R022
- M026 leaves R022 active and out of scope for validation closeout.
- RLM document navigation and workflow prototypes are outside M026 loader-research scope.
### R023
- M026 leaves R023 active and out of scope for validation closeout.
- RLM graph traversal benchmark adoption decisions are outside M026 loader-research scope.
### R024
- M026 leaves R024 active and out of scope for validation closeout.
- Staged 10-document, 20-document, and one-week Scientific KG behavior validation remains future work.
### R027
- M026 leaves R027 active and out of scope for validation closeout.
- Graph-readiness quality acceptance remains future work; M026 reports only inform future loader evidence design.
### R029
- M026 leaves R029 active and out of scope for validation closeout.
- Import-ready typed chunk packages remain future work; M026 does not authorize import readiness.
### R031
- M026 leaves R031 active and out of scope for validation closeout.
- 30-paper deviation scans are outside M026 loader-research scope.
### R032
- M026 leaves R032 active and out of scope for validation closeout.
- Automated +10-paper loops toward 100 papers are outside M026 loader-research scope.
### R033
- M026 leaves R033 active and out of scope for validation closeout.
- Deterministic iterative validation CLI implementation is outside M026 loader-research scope.
### R035
- M026 leaves R035 active and out of scope for validation closeout.
- Accepted-paper quota top-up implementation is outside M026 loader-research scope.
### R036
- M026 advances artifact-level traceability with cited reports, compatibility evidence, validation notes, and gsd_exec verification evidence.
- M026 records adjacent provenance evidence without claiming full validation CLI provenance coverage.
### R040
- M026 followed R040 for this milestone by researching source capabilities, probing compatibility, documenting failure modes, and safety-wrapping loader boundaries before any main-process enablement.
- M026 provides milestone-local constraint evidence for loader research only.
### R050
- M026 informs future R050 loader and evidence-bundle design.
- R050 remains a future consumer of M026 contracts, not implemented by M026.
### R051
- M026 leaves R051 active and out of scope for validation closeout.
- MiniMax bounded helper activation remains gated and outside M026.
### R052
- M026 leaves R052 active and out of scope for validation closeout.
- DSPy prompt optimization remains gated and outside M026.
### R030
- R030 is already validated by prior source-artifact preservation evidence.
- M026 supports future preservation by requiring references, checksums, provenance, redaction, and metadata-only boundaries.

## Forbidden Validation Language
### Global
- M026 implements the loader
- M026 implements batch selection
- M026 authorizes KG import
- M026 validates KG import readiness
- M026 validates graph readiness
- M026 validates Scientific KG corpus behavior
- M026 validates import-ready chunks
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
- M026 embeds raw article text, PDFs, binary payloads, vector payloads, secrets, or production connection details
- M026 writes to production LadybugDB
### R001
- M026 newly validates R001
- M026 changes the canonical status or scope of R001
### R002
- M026 newly validates R002
- M026 changes the canonical status or scope of R002
### R003
- M026 newly validates R003
- M026 changes the canonical status or scope of R003
### R004
- M026 newly validates R004
- M026 changes the canonical status or scope of R004
### R005
- M026 newly validates R005
- M026 changes the canonical status or scope of R005
### R006
- M026 newly validates R006
- M026 changes the canonical status or scope of R006
### R007
- M026 newly validates R007
- M026 changes the canonical status or scope of R007
### R008
- M026 newly validates R008
- M026 changes the canonical status or scope of R008
### R009
- M026 newly validates R009
- M026 changes the canonical status or scope of R009
### R010
- M026 newly validates R010
- M026 changes the canonical status or scope of R010
### R014
- M026 newly validates R014
- M026 changes local full-text ingestion behavior
- M026 embeds raw article text in digest or contract artifacts
### R019
- M026 fully validates R019
- M026 changes R019 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R022
- M026 fully validates R022
- M026 changes R022 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R023
- M026 fully validates R023
- M026 changes R023 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R024
- M026 fully validates R024
- M026 changes R024 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R027
- M026 fully validates R027
- M026 changes R027 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R029
- M026 fully validates R029
- M026 changes R029 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R031
- M026 fully validates R031
- M026 changes R031 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R032
- M026 fully validates R032
- M026 changes R032 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R033
- M026 fully validates R033
- M026 changes R033 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R035
- M026 fully validates R035
- M026 changes R035 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R036
- M026 fully validates R036
- M026 proves every validation CLI records exact command, inputs, output hashes, exit code, cwd, git commit, and active milestone/batch context
- M026 authorizes KG import readiness
- M026 validates graph readiness
### R040
- M026 globally validates R040 for all future infrastructure
- M026 enables loader infrastructure in the main Scientific KG process
- M026 authorizes production LadybugDB writes
- M026 authorizes KG import readiness
### R050
- M026 implements R050
- M026 provides a deterministic article structure artifact detection CLI
- M026 emits candidate KG scaffold links
- M026 authorizes KG import readiness
### R051
- M026 fully validates R051
- M026 changes R051 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R052
- M026 fully validates R052
- M026 changes R052 to validated
- M026 authorizes KG import readiness
- M026 validates graph readiness
- M026 authorizes production LadybugDB writes
- M026 activates DSPy, RLM, MiniMax, or optimizer behavior
### R030
- M026 newly validates R030 beyond existing canonical validation
- M026 embeds raw PDFs, raw article text, binary payloads, vectors, secrets, or production connection details
- M026 implements multimodal retrieval or graph import for preserved artifacts

## Remaining Work
### R001
- No R001 status change is needed for M026.
### R002
- No R002 status change is needed for M026.
### R003
- No R003 status change is needed for M026.
### R004
- No R004 status change is needed for M026.
### R005
- No R005 status change is needed for M026.
### R006
- No R006 status change is needed for M026.
### R007
- No R007 status change is needed for M026.
### R008
- No R008 status change is needed for M026.
### R009
- No R009 status change is needed for M026.
### R010
- No R010 status change is needed for M026.
### R014
- No R014 status change is needed for M026.
### R019
- Satisfy R019 in a future milestone with evidence matching the canonical requirement wording.
### R022
- Satisfy R022 in a future milestone with evidence matching the canonical requirement wording.
### R023
- Satisfy R023 in a future milestone with evidence matching the canonical requirement wording.
### R024
- Satisfy R024 in a future milestone with evidence matching the canonical requirement wording.
### R027
- Satisfy R027 in a future milestone with evidence matching the canonical requirement wording.
### R029
- Satisfy R029 in a future milestone with evidence matching the canonical requirement wording.
### R031
- Satisfy R031 in a future milestone with evidence matching the canonical requirement wording.
### R032
- Satisfy R032 in a future milestone with evidence matching the canonical requirement wording.
### R033
- Satisfy R033 in a future milestone with evidence matching the canonical requirement wording.
### R035
- Satisfy R035 in a future milestone with evidence matching the canonical requirement wording.
### R036
- Implement or prove full R036 field-level provenance for the relevant validation CLI surfaces before changing canonical status.
### R040
- Continue applying R040 to future infrastructure decisions before activation.
### R050
- Implement the deterministic artifact-detection CLI with no-import safety gates in a later milestone.
### R051
- Satisfy R051 in a future milestone with evidence matching the canonical requirement wording.
### R052
- Satisfy R052 in a future milestone with evidence matching the canonical requirement wording.
### R030
- No R030 status change is needed for M026.

## Failure Modes
- Filesystem dependency: source artifacts or output directory may be missing; verifier must fail with named missing paths rather than assuming coverage.
- JSON dependency: malformed JSON must fail parsing before validation claims are trusted.
- Markdown render dependency: stale markdown must fail if any required requirement id is absent.
- No network, API, database, or subprocess runtime dependency is introduced by these static artifacts.

## Load Profile

Static closeout artifacts have no runtime load dimension. The only 10x case is a larger requirement list, which saturates human review attention before compute; protection is the machine-readable JSON row list and verifier expectations for missing/stale rows.

## Negative Tests
- Missing requirement rows are rejected.
- Malformed or unsafe evidence paths are rejected.
- Unsafe positive phrases are rejected outside forbidden-claim fields.
- Unsafe true booleans in safety_flags are rejected.
- Raw/binary/vector/secret/production connection field drift is rejected.

## Observability Impact
The matrix is a machine-readable closeout diagnostic surface for future validators; it names classifications, evidence paths, allowed claims, forbidden claims, and remaining work without changing runtime observability.

## Verifier Expectations
- Reject missing requirement rows for any required_requirement_ids entry.
- Reject stale rendered markdown that omits any JSON requirement row.
- Reject evidence paths that are absolute, point outside the repository, contain traversal segments, or have unsupported extensions.
- Reject unsafe positive claims outside forbidden-claim fields.
- Reject unsafe true booleans in safety_flags.
- Reject raw, binary, vector, secret, or production connection field drift.

## Safety Flags
- `loader_implementation_claimed`: `false`
- `batch_selection_implementation_claimed`: `false`
- `kg_import_or_readiness_claimed`: `false`
- `graph_validation_claimed`: `false`
- `import_ready_chunks_claimed`: `false`
- `scientific_kg_corpus_validation_claimed`: `false`
- `dspy_rlm_minimax_activation_claimed`: `false`
- `raw_payloads_embedded`: `false`
- `binary_payloads_embedded`: `false`
- `vector_payloads_embedded`: `false`
- `secrets_embedded`: `false`
- `production_ladybugdb_writes_claimed`: `false`

## Handoff for Milestone Validation
Use `doc/validation/m026_requirement_scope_matrix.json` as the machine-checkable source of truth and this Markdown file as the human-readable closeout language guide. Only R040 is in-scope as a followed constraint for M026, R036 is adjacent evidence but not fully validated, R050 is a future consumer, R014/R030/R001-R010 are existing coverage or compatibility context, and all other active requirements remain out of scope.
