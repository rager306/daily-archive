# M034 R/D Consistency Audit

Generated: 2026-06-06T07:44:41.337740+00:00

## Architecture Frame Checked
- north_star: local-first universal knowledge base
- primary_domain: scientific articles
- graphdb_selection: deferred
- sidecar_outputs: candidate evidence only
- agents: optional future workers, not current core orchestrator

## Classification Counts
- consistent: 35
- historical-scope-only: 78
- needs-clarification: 15

## Flag Counts
- agent-helper-scope-broad: 1
- ladybugdb-reference: 20
- paper-domain-scope: 80
- paper-sidecar-scope: 8

## Findings Needing Clarification or User Decision
| ID | Kind | Classification | Flags | Recommended Route | Finding |
|---|---|---|---|---|---|
| R019 | requirement | needs-clarification | none | S03/S04 ADR clarification | Hybrid retrieval evidence requirement remains valid, but future ADRs should avoid assuming a specific graph substrate implementation. |
| R024 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | S03/S04 ADR clarification | Scientific KG staged validation remains primary-domain scope; ADRs should clarify it is the proving path, not the only future KB domain. |
| R027 | requirement | needs-clarification | paper-domain-scope | S03/S04 ADR clarification | Graph-readiness quality contract remains binding for scientific papers; ADRs should separate paper-specific readiness from generic KB readiness. |
| R029 | requirement | needs-clarification | paper-domain-scope | S03/S04 ADR clarification | Chunk import-ready package remains scientific-paper specific; ADRs should avoid treating it as universal content contract. |
| R031 | requirement | needs-clarification | paper-domain-scope | S03/S04 ADR clarification | 30-paper deviation scan remains paper-domain validation breadth; ADRs should keep it as primary-domain evidence, not universal-KB completeness. |
| R033 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | S03/S04 ADR clarification | Deterministic corpus CLI remains compatible; clarify that it is a paper-domain workflow over generic resumable evidence orchestration. |
| R050 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | S03/S04 ADR clarification | Article-structure CLI remains primary-domain capability; universal-KB ADRs should generalize candidate detection without weakening no-import safety. |
| R056 | requirement | needs-clarification | ladybugdb-reference, paper-sidecar-scope | S03/S04 ADR clarification | Mentions LadybugDB-specific flag; future contracts should generalize to graphdb_written while preserving ladybugdb_written for historical compatibility. |
| R058 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | S03/S04 ADR clarification | North-star wording says scientific paper evidence chains; superseding ADR should broaden to universal KB with scientific articles as first domain. |
| R059 | requirement | needs-clarification | ladybugdb-reference | S03/S04 ADR clarification | Correctly defers GraphDB choice; clarify that LadybugDB references are candidate/early-substrate, not final selection. |
| R061 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | S03/S04 ADR clarification | Audit requirement is compatible; clarify it covers universal-KB, GraphDB deferral, paper-domain boundaries, and safety invariants. |
| D012 | decision | needs-clarification | none | S03 superseding/clarifying ADR language | Mentions LadybugDB-oriented KG progression historically; clarify as early substrate/import-model milestone, not final GraphDB selection. |
| D036 | decision | needs-clarification | paper-domain-scope, agent-helper-scope-broad | S03 superseding/clarifying ADR language | Broad wording “MiniMax may be used wherever it helps” should be narrowed by bounded helper, redaction, and non-authoritative output constraints in universal-KB ADRs. |
| D061 | decision | needs-clarification | ladybugdb-reference, paper-domain-scope, paper-sidecar-scope | S03 superseding/clarifying ADR language | Vendoring GROBID/OpenDataLoader remains valid research input; clarify it does not imply production parser adoption or final graph substrate. |
| D062 | decision | needs-clarification | paper-domain-scope, paper-sidecar-scope | S03 superseding/clarifying ADR language | External parser research scope is paper-specific; keep as primary-domain evidence rather than universal-KB-only architecture. |

## Full Classification Table
| ID | Kind | Classification | Flags | Title |
|---|---|---|---|---|
| R019 | requirement | needs-clarification | none | Hybrid retrieval must return traceable evidence contexts with vector, graph, fusion score metadata, and EvidencePath references. |
| R022 | requirement | consistent | none | RLM document navigation and workflow-in-code prototypes must be read-only, bounded, and return typed draft outputs plus trajectories validated by deterministic  |
| R023 | requirement | consistent | none | RLM graph traversal must be benchmarked against vector-only, one-hop graph expansion, and heuristic BFS before adoption recommendations are made. |
| R024 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | Before expanding beyond M003, the system must validate current scientific KG behavior on staged real article batches of 10 documents, 20 documents, and then a o |
| R027 | requirement | needs-clarification | paper-domain-scope | Before scientific KG validation or scaling continues, converted paper data and chunks must satisfy an explicit graph-readiness quality contract covering convers |
| R029 | requirement | needs-clarification | paper-domain-scope | Before KG import continues, chunking must produce an import-ready typed chunk package with stable IDs, source spans, parent-child lineage, content routes, quali |
| R031 | requirement | needs-clarification | paper-domain-scope | Before drawing broader chunking/import-readiness conclusions, validation must expand from the 10-paper gold corpus to a 30-paper deviation scan that compares di |
| R032 | requirement | consistent | paper-domain-scope | Corpus validation must support an automated +10-paper iterative loop up to 100 papers, with resumable batch state, source acquisition, deviation analysis, remed |
| R033 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | Provide a deterministic, resumable CLI workflow for iterative +10-paper validation batches toward a 100-paper diagnostic corpus. |
| R035 | requirement | consistent | paper-domain-scope | Validation batches must fill the target accepted-paper quota by drawing deterministic replacement candidates when selected papers cannot become source-ready wit |
| R040 | requirement | consistent | paper-domain-scope | New infrastructure must be researched, compatibility-probed, and safety-wrapped before it is enabled in the main Scientific KG process. |
| R050 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | Provide a deterministic CLI for detecting article structure artifacts and candidate KG scaffold links from preserved paper sources without performing KG import. |
| R051 | requirement | consistent | paper-domain-scope | MiniMax may assist article artifact detection only as a bounded structured helper with forced tool calls, local schema validation, redacted inputs, and non-auth |
| R052 | requirement | consistent | none | DSPy prompt optimization for artifact detection must remain gated until benchmark fixtures, metrics, and baseline MiniMax or deterministic outputs exist. |
| R054 | requirement | consistent | paper-domain-scope, paper-sidecar-scope | Provide a durable lazy async sidecar pipeline for article processing jobs. |
| R055 | requirement | consistent | paper-sidecar-scope | Track sidecar job lifecycle, retries, typed blockers, and backend/cache health explicitly. |
| R056 | requirement | needs-clarification | ladybugdb-reference, paper-sidecar-scope | Parser sidecar outputs must remain candidate evidence until daily-archive validators, review packets, and graph-readiness review pass. |
| R057 | requirement | consistent | none | Future sidecar-pipeline roadmap must include explicit architecture brainstorm and decision gates at key irreversible points. |
| R058 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | Post-M033 ADR package must root every sidecar and orchestration decision in the overall daily-archive mission: local-first scientific paper evidence chains befo |
| R059 | requirement | needs-clarification | ladybugdb-reference | Do not lock the future knowledge graph database choice to LadybugDB before a dedicated GraphDB evaluation compares viable local-first candidates. |
| R060 | requirement | consistent | paper-domain-scope | Frame the architecture around a universal local-first knowledge base, with scientific articles as the primary current domain and proving ground. |
| R061 | requirement | needs-clarification | ladybugdb-reference, paper-domain-scope | M034 must audit all existing GSD requirements and decisions for mutual consistency with the universal knowledge-base ADR package before closeout. |
| R001 | requirement | historical-scope-only | none | CLI help info |
| R002 | requirement | historical-scope-only | paper-domain-scope | CLI `--date` analysis |
| R003 | requirement | historical-scope-only | none | JSON result in sessions |
| R004 | requirement | historical-scope-only | paper-domain-scope | Save full list of papers |
| R005 | requirement | historical-scope-only | paper-domain-scope | Per-paper artifacts |
| R006 | requirement | historical-scope-only | paper-domain-scope | Topic overview aggregates |
| R007 | requirement | historical-scope-only | paper-domain-scope | Transparent score breakdown |
| R008 | requirement | historical-scope-only | none | Queue state file |
| R009 | requirement | historical-scope-only | none | Idempotent reruns |
| R010 | requirement | historical-scope-only | none | Rust-portable contracts |
| R011 | requirement | historical-scope-only | none | Follow style guide/lint |
| R012 | requirement | historical-scope-only | paper-domain-scope | Empty day handling |
| R013 | requirement | historical-scope-only | none | Pytest contract coverage |
| R014 | requirement | historical-scope-only | paper-domain-scope | Local full-text ingestion must produce deterministic, provenance-rich ingestion results for markdown and plain-text paper artifacts. |
| R015 | requirement | historical-scope-only | paper-domain-scope | PageIndex document navigation must represent fixture papers as deterministic PageIndexNode hierarchies with parent, child, NEXT, path, and validation diagnostic |
| R016 | requirement | historical-scope-only | paper-domain-scope | SemanticChunk records and EvidencePath objects must provide deterministic traceability from Paper to PageIndexNode to chunk. |
| R017 | requirement | historical-scope-only | paper-domain-scope | Claim, ScientificEntity, and ScientificRelation contracts must reference EvidencePath records or explicit validation errors. |
| R018 | requirement | historical-scope-only | ladybugdb-reference, paper-domain-scope | LadybugDB SCI KG schema must store Paper, PageIndexNode, SemanticChunk, Claim, ScientificEntity, ScientificRelation, EvidencePath, and required edges idempotent |
| R020 | requirement | historical-scope-only | paper-domain-scope | Evaluation fixtures and metrics must exist before scale, optimizer, DSPy, RLM, or retrieval-quality claims are made. |
| R021 | requirement | historical-scope-only | paper-domain-scope | DSPy extraction boundaries must remain disabled or non-optimizing until evaluation metrics and benchmark fixtures are verified. |
| R025 | requirement | historical-scope-only | paper-domain-scope | Full-text acquisition and real-corpus KG validation must emit structured Loguru-based logs and persisted diagnostics for each selected paper before rerunning th |
| R026 | requirement | historical-scope-only | ladybugdb-reference, paper-domain-scope | Before scaling validation to 10, 20, or larger document batches, the real-data scientific KG pipeline must be debugged end-to-end on the current small corpus th |
| R028 | requirement | historical-scope-only | ladybugdb-reference, paper-domain-scope | Validation of conversion, chunking, extraction, and graph-readiness must include an independent artifact review step where feasible, preferably via a subagent,  |
| R030 | requirement | historical-scope-only | paper-domain-scope | Article ingestion must preserve source artifacts alongside derived text, including the original PDF, normalized Markdown, extracted figures, tables, image asset |
| R034 | requirement | historical-scope-only | paper-domain-scope | Run the first genuinely new +10-paper validation batch through the deterministic M007 validation-batch workflow. |
| R036 | requirement | historical-scope-only | ladybugdb-reference | Validation CLI runs must produce replay/audit provenance logs tying each generated artifact to the exact command, inputs, output hashes, exit code, cwd, git com |
| R037 | requirement | historical-scope-only | paper-domain-scope | Run the next reviewed +10 validation batch using M009 runbook gates: active scan lineage, real provenance entry, artifact freshness verification, and bounded to |
| R038 | requirement | historical-scope-only | ladybugdb-reference | Before any positive KG import, a reviewed semantic evidence gate must evaluate a small subset of scanned chunks/outliers for factual extraction readiness withou |
| R039 | requirement | historical-scope-only | paper-domain-scope | Before enabling DSPy or MiniMax in the Scientific KG pipeline, the project must complete parallel compatibility research proving version/API requirements, minim |
| R041 | requirement | historical-scope-only | paper-domain-scope | Before any DSPy optimizer or MiniMax helper is used in the Scientific KG process, the project must prove detailed optimizer applicability, dependency/install fe |
| R042 | requirement | historical-scope-only | ladybugdb-reference | MiniMax advancement must use real bounded API tests, document Token Plan quota/limit visibility, and keep external calls redacted and non-authoritative. |
| R043 | requirement | historical-scope-only | none | MiniMax remediation must prove Token Plan limit-check access and structured JSON behavior using the correct API surfaces before any helper verdict is accepted. |
| R044 | requirement | historical-scope-only | none | MiniMax limit checking must follow the 9router endpoint/fallback/parsing algorithm before declaring Token Plan remains unresolved. |
| R045 | requirement | historical-scope-only | ladybugdb-reference, paper-domain-scope | MiniMax helper implementation must turn the verified MiniMax findings into dev-only, tested code before MiniMax is used in any Scientific KG workflow. |
| R046 | requirement | historical-scope-only | paper-domain-scope | Triage ML dependency vulnerabilities before enabling any runtime path that uses vulnerable ML packages. |
| R047 | requirement | historical-scope-only | ladybugdb-reference, paper-domain-scope | Compare selected open-source research-agent systems before adopting any research-agent patterns into the Scientific KG workflow. |
| R048 | requirement | historical-scope-only | ladybugdb-reference, paper-domain-scope | Define and validate KG candidate locators with chunk-span provenance before any positive Scientific KG import is allowed. |
| R049 | requirement | historical-scope-only | ladybugdb-reference, paper-domain-scope | Implement deterministic candidate locator generation with schema validation, source hash checks, coordinate validation, safety guards, and ambiguity diagnostics |
| R053 | requirement | historical-scope-only | ladybugdb-reference, paper-domain-scope, paper-sidecar-scope | External PDF/parser tool evaluation must compare current daily-archive outputs with opendataloader-pdf and GROBID on a bounded local PDF corpus before any parse |
| D001 | decision | consistent | none | When to introduce DSPy extraction in M003 |
| D002 | decision | historical-scope-only | none | How to introduce the DSPy extraction boundary |
| D003 | decision | historical-scope-only | none | S09 RLM document/workflow harness interface |
| D004 | decision | historical-scope-only | paper-domain-scope | S10 RLM graph traversal boundary |
| D005 | decision | consistent | paper-domain-scope | Pause feature expansion after M003 and validate current scientific knowledge graph quality on real article batches before moving forward. |
| D006 | decision | historical-scope-only | paper-domain-scope | How to proceed after M004/S01 found missing full-text artifacts for all 10 selected papers. |
| D007 | decision | historical-scope-only | paper-domain-scope | Which logging library to use for M004 full-text bridge diagnostics |
| D008 | decision | historical-scope-only | paper-domain-scope | What should happen after M004/S02 produced full text and evidence paths but no real SCI KG persistence. |
| D009 | decision | historical-scope-only | paper-domain-scope | PDF-to-Markdown fallback strategy for M004 validation |
| D010 | decision | consistent | paper-domain-scope | Gate M004 progression on graph-ready data preparation quality |
| D011 | decision | consistent | paper-domain-scope | Add independent result recheck as a validation practice |
| D012 | decision | needs-clarification | none | How to proceed after M004 exact-ID retrieval validation |
| D013 | decision | historical-scope-only | paper-domain-scope | How to handle raw article sources and visual assets during chunking/import-model work |
| D014 | decision | historical-scope-only | paper-domain-scope | How to respond to the user's concern that the 10-paper corpus is not representative enough |
| D015 | decision | consistent | paper-domain-scope | How to automate iterative validation from 30 toward 100 papers and whether MiniMax should drive the loop |
| D016 | decision | consistent | paper-domain-scope | How to continue after M006's reviewed 30-paper deviation scan |
| D017 | decision | historical-scope-only | paper-domain-scope | How to proceed after M007 validation-batch workflow automation |
| D018 | decision | historical-scope-only | paper-domain-scope | How validation-batch automation should behave when the initially selected papers do not yield the required number of source-ready papers |
| D019 | decision | historical-scope-only | none | What to do after M008 review flagged weak CLI provenance and missing shortage top-up behavior |
| D020 | decision | historical-scope-only | none | How to proceed after M009 validation CLI hardening |
| D021 | decision | consistent | paper-domain-scope | How to continue after M010's reviewed provenance-gated plus-ten batch |
| D022 | decision | consistent | none | How to prepare for future DSPy and MiniMax adoption after M011 |
| D023 | decision | consistent | paper-domain-scope | How new infrastructure is introduced into the Scientific KG pipeline |
| D024 | decision | historical-scope-only | none | How to execute independent infrastructure research tasks |
| D025 | decision | historical-scope-only | none | Authoritative starting sources for M012 DSPy and MiniMax compatibility research |
| D026 | decision | historical-scope-only | none | DSPy research source scope for M012 |
| D027 | decision | historical-scope-only | paper-domain-scope | How to continue after M012 compatibility research |
| D028 | decision | historical-scope-only | paper-domain-scope | How to proceed after M013 MiniMax synthetic smoke-test success |
| D029 | decision | historical-scope-only | none | How to respond to M014 being under-debugged |
| D030 | decision | historical-scope-only | none | How to remediate MiniMax Token Plan limit checking after M015 |
| D031 | decision | historical-scope-only | none | What to do after documenting MiniMax findings globally |
| D032 | decision | historical-scope-only | none | How to address M017 dependency-audit debt |
| D033 | decision | historical-scope-only | none | Next GSD milestone after MiniMax helper and dependency-debt triage |
| D034 | decision | consistent | paper-domain-scope | How to resume Scientific KG work after research-agent spike |
| D035 | decision | historical-scope-only | none | What KG work should follow M020 candidate locator protocol validation |
| D036 | decision | needs-clarification | paper-domain-scope, agent-helper-scope-broad | Use MiniMax as an available local-first helper across the article evidence pipeline and add riskratchet as a non-blocking diagnostic gate in GSD review cycles,  |
| D037 | decision | historical-scope-only | paper-domain-scope | Define S01 article loader as a local source loading and classification boundary rather than an acquisition or conversion pipeline. |
| D038 | decision | historical-scope-only | paper-domain-scope | M024 S03 PageIndex navigation implementation boundary |
| D039 | decision | historical-scope-only | paper-domain-scope | M024 S04 asset preservation contract module boundary |
| D040 | decision | consistent | paper-domain-scope | Represent article links, metadata signals, and deduplication as a dedicated first-class bridge subtree rather than overloading retrieval or metrics placeholders |
| D041 | decision | historical-scope-only | paper-domain-scope | Represent retrieval and table benchmarks as a dedicated metadata-only contract module with aggregate-only bridge attachment. |
| D042 | decision | consistent | paper-domain-scope | M025 article pipeline refactor must be grounded by real-article replay before and during module extraction. |
| D043 | decision | consistent | paper-domain-scope | M025 article catalog source strategy prioritizes HTML/Markdown while preserving PDF variants immediately. |
| D044 | decision | historical-scope-only | none | How riskratchet participates in coding and validation workflow |
| D045 | decision | historical-scope-only | paper-domain-scope | How to resolve stale M027 working-state references before running GSD auto |
| D046 | decision | historical-scope-only | none | Shape the S05 end-to-end mixed replay boundary as a replay command plus validate-only verifier rather than mutating the S04 baseline or splitting into many stag |
| D047 | decision | historical-scope-only | none | How S06 should add provenance and maintainability telemetry without mutating completed replay slices |
| D048 | decision | historical-scope-only | paper-domain-scope | How to resolve M027-aakeky needs-attention validation before milestone closeout |
| D049 | decision | historical-scope-only | paper-domain-scope | How M028 should handle the expanded user-supplied corpus before auto-mode resumes |
| D050 | decision | consistent | paper-domain-scope | How S03 should attempt PDF acquisition diagnostics for the smoke corpus |
| D051 | decision | historical-scope-only | paper-domain-scope | How M028 S07 should remediate requirement coverage alignment |
| D052 | decision | historical-scope-only | paper-domain-scope | Whether M029 Unified Article Corpus Load should execute before or after M030 Pipeline Architecture Inventory and Continuity Audit |
| D053 | decision | consistent | paper-domain-scope | How to treat Scrapling in the daily-archive article-processing pipeline readiness assessment |
| D054 | decision | consistent | none | How to proceed with parked M029 after M030 validation and M031 planning |
| D055 | decision | historical-scope-only | paper-domain-scope | How should S02 expose catalog-backed acquisition and loader replay to downstream slices? |
| D056 | decision | consistent | paper-domain-scope | How S03 should perform parser and conversion replay for catalog-backed local artifacts. |
| D057 | decision | consistent | none | How S04 should expose automated graph-readiness output for chunked converted artifacts |
| D058 | decision | consistent | none | How should M031 S05 map S04 chunk and graph-readiness artifacts into import-boundary rehearsal evidence? |
| D059 | decision | historical-scope-only | none | How should M031 S05 produce the final progression matrix and continuity checkpoint? |
| D060 | decision | historical-scope-only | none | How M031 S06 should resolve stale S02 assessment evidence for milestone validation |
| D061 | decision | needs-clarification | ladybugdb-reference, paper-domain-scope, paper-sidecar-scope | How to evaluate external PDF parser tools after M031 |
| D062 | decision | needs-clarification | paper-domain-scope, paper-sidecar-scope | Scope of the next external article-processing research track |
| D063 | decision | consistent | paper-sidecar-scope | Post-M033 parser architecture direction |
| D064 | decision | consistent | none | Agentic pipeline adoption boundary |
| D065 | decision | consistent | ladybugdb-reference | Graph database selection status for the knowledge layer |
| D066 | decision | consistent | paper-domain-scope, paper-sidecar-scope | Overall knowledge system direction after M033 |
| D067 | decision | consistent | none | ADR template for M034 decision package |
