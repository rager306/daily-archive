# Handoff — 2026-08-11 (M363 S01-S02 delivered, S03 planned)

## Active milestone

**M363-ma2kmw** — article preparation agent readiness.

- S01 skipped-delivered + runtime UAT PASS.
- S02 skipped-delivered + runtime UAT PASS.
- S03 planned, pending.
- S04 pending.

## Delivered in this session

### S01 revision-safe grounded summaries — `464ef9c2`

Fixed a real revision-history bug:
- old Summary VID used `paper_id + section path_id`;
- v1/v5 shared path IDs and could overwrite historical summary evidence;
- new VID is SHA256 over exact `PaperRevision VID + granularity + source VID`;
- abstract digest owner is PaperRevision; Section digest owner is exact revision Section;
- materializer validates `(label, vid) -> node_id` for revision and every Section before writes;
- replaced vacuous `let _ = err` test with missing-revision and missing-Section fail-closed tests;
- RevisionMaterializationUseCase now writes `canonical_id` and revision-local `abstract_text`;
- production ontology DATA + compiled edge parity includes `PaperRevision -[:hasSummary]-> Summary`.

Real Samyama: 2 revisions, 24 distinct summaries, replay stable, D127 false, base tasks PASS.

### S02 DATA-driven semantic roles — `9d69ba09`

- New pure domain `SectionRolePolicy`; no Methods/Results title vocabulary in application/CLI.
- Production DATA: `data/ontology/section_summary_roles.yaml`.
- Adapter loads bundled/project YAML and fingerprints canonical loaded values.
- Validated open `SummaryType` wire strings; exact/suffix rules, duplicate rejection, serde.
- `max_title_chars=160` prevents paragraph-sized parser titles from role classification.
- Generic `Summary` nodes use `summary_type=methods|results|generic`, no new labels.
- Nodes bind role policy schema + SHA; replay without policy resets stale role/lineage.
- Fixture proves methods+results+generic and policy removal replay.

Real exact v1/v5 paper is legal ontology content:
- revisions=2;
- summaries=24;
- methods=2;
- results=0 (honest missingness; no Results section);
- generic=22;
- malformed parser titles remain generic evidence;
- semantic tasks=5 PASS, EMPTY=0, ERROR=0;
- D127=false.

UAT evidence:
- S01 `2e2848c0-3f46-48cb-be03-d7d5e6c49cf2`
- S02 `5d48d5c8-ded1-4d85-bda9-1a6bee4ec490`

Full `scripts/verify_rust_architecture.sh`: PASS. Focused all-target clippy: 0 project errors. Governor: zero violations; generated trajectory report remains unstaged.

## Next action — S03

Implement planned **revision-scoped readiness QualitySignal** thinly:

1. TDD `revision_readiness` domain observation/policy:
   - exact section/summary/methods/results/manifestation counts;
   - integer ratio (basis points), zero denominator safe;
   - `ready|partial|blocked`;
   - blockers separate from diagnostics;
   - serde + property + negative tests.
2. Add production `data/ontology/revision_readiness_policy.yaml` + adapter canonical-value SHA.
3. Application reads one explicit PaperRevision neighborhood and materializes one QualitySignal with exact evidence digest, policy lineage, state/reasons, D127 false.
4. Add additive `PaperRevision -[:hasQualitySignal]-> QualitySignal`; retain legacy Paper endpoint.
5. Use temporal fact helper for edge; real v1/v5 canary and task suite; replay unchanged.

Decision D225: legacy LLM `reproducibility_score` is not readiness authority. No network/LLM. Results absence is a diagnostic reason, never invented evidence.

## Governor assessment

Do not add a regex now. The session bugs (revision identity collision, vacuous negative test) are better prevented by product contract tests and ontology endpoint audit. After S03 has a negative fixture, consider a DATA-backed ratchet rejecting Paper-scoped `agent_readiness` signals when revision evidence exists.

## Intelligence notes

- GitNexus reindexed after S01 and S02; authoritative impact/detect_changes used.
- Codebase Memory MCP timed out on all narrow searches; recorded as supplemental unavailable.
- GitNexus briefly exited during reindex, then recovered.

## Locks

D127 false; ADR-057 Proposed; ADR-058 Accepted; no DSPy enablement; Samyama sole graph/vector/persist store; cache and QualitySignal are derived evidence, never graph truth.
