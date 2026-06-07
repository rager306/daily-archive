# M034 Open Conflicts for User

Generated: 2026-06-06T07:45:29.997492+00:00

No immediate `conflict-needs-user-decision` records were found after refining false positives around no-import/no-write wording.

## Clarifications to Preserve in ADRs

These do not currently require a blocking user answer, but the ADR package must address them explicitly:

| ID | Kind | Clarification Needed | Findings |
|---|---|---|---|
| R019 | requirement | Clarify GraphDB portability and avoid LadybugDB finality. | Hybrid retrieval evidence requirement remains valid, but future ADRs should avoid assuming a specific graph substrate implementation. |
| R024 | requirement | Clarify paper-domain scope under universal-KB north star. | Scientific KG staged validation remains primary-domain scope; ADRs should clarify it is the proving path, not the only future KB domain. |
| R027 | requirement | Clarify paper-domain scope under universal-KB north star. | Graph-readiness quality contract remains binding for scientific papers; ADRs should separate paper-specific readiness from generic KB readiness. |
| R029 | requirement | Clarify paper-domain scope under universal-KB north star. | Chunk import-ready package remains scientific-paper specific; ADRs should avoid treating it as universal content contract. |
| R031 | requirement | Clarify paper-domain scope under universal-KB north star. | 30-paper deviation scan remains paper-domain validation breadth; ADRs should keep it as primary-domain evidence, not universal-KB completeness. |
| R033 | requirement | Clarify paper-domain scope under universal-KB north star. | Deterministic corpus CLI remains compatible; clarify that it is a paper-domain workflow over generic resumable evidence orchestration. |
| R050 | requirement | Clarify paper-domain scope under universal-KB north star. | Article-structure CLI remains primary-domain capability; universal-KB ADRs should generalize candidate detection without weakening no-import safety. |
| R056 | requirement | Clarify GraphDB portability and avoid LadybugDB finality. | Mentions LadybugDB-specific flag; future contracts should generalize to graphdb_written while preserving ladybugdb_written for historical compatibility. |
| R058 | requirement | Clarify paper-domain scope under universal-KB north star. | North-star wording says scientific paper evidence chains; superseding ADR should broaden to universal KB with scientific articles as first domain. |
| R059 | requirement | Clarify GraphDB portability and avoid LadybugDB finality. | Correctly defers GraphDB choice; clarify that LadybugDB references are candidate/early-substrate, not final selection. |
| R061 | requirement | Carry audit obligation into closeout and future ADR index. | Audit requirement is compatible; clarify it covers universal-KB, GraphDB deferral, paper-domain boundaries, and safety invariants. |
| D012 | decision | Clarify GraphDB portability and avoid LadybugDB finality. | Mentions LadybugDB-oriented KG progression historically; clarify as early substrate/import-model milestone, not final GraphDB selection. |
| D036 | decision | Narrow broad helper wording to bounded, redacted, non-authoritative helpers. | Broad wording “MiniMax may be used wherever it helps” should be narrowed by bounded helper, redaction, and non-authoritative output constraints in universal-KB ADRs. |
| D061 | decision | Clarify GraphDB portability and avoid LadybugDB finality. | Vendoring GROBID/OpenDataLoader remains valid research input; clarify it does not imply production parser adoption or final graph substrate. |
| D062 | decision | Clarify paper-domain scope under universal-KB north star. | External parser research scope is paper-specific; keep as primary-domain evidence rather than universal-KB-only architecture. |
