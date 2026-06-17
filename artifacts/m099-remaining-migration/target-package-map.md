# M099 Target Package Map

Source inventory: `artifacts/m099-remaining-migration/remaining-module-inventory.md`.

Goal: retire remaining production runtime code from `src/arxiv_archive` without permanent shims, preserving old implementations under `archive/package-rename-waves/wave-XX/` with `Formerly:` breadcrumbs and manifest rows.

## Migration Order

| Order | Slice | Cluster | Why this order |
|---:|---|---|---|
| 1 | S02 | `identity` | Leaf primitives used by staging and later workflow boundaries. |
| 2 | S02 | `staging` | Depends on identity; consumed by graph/universal KB workflows. |
| 3 | S03 | `quality` | Mostly tooling/diagnostic leaf package; low runtime fan-in. |
| 4 | S04 | `repair_chunks` | Contract and repair code with local-only diagnostic constraints. |
| 5 | S05 | `extraction_evaluation` | Higher risk because it includes DSPy/provider-adjacent code; must remain deterministic/local. |
| 6 | S06 | `retrieval_embedding` | Consumes evaluation/retrieval primitives; keep fixture-level baseline semantics. |
| 7 | S07 | `external_clients` | Side-effect boundaries must be made explicit and mocked before workflow moves. |
| 8 | S08 | `validation_batch` | Workflow/state modules depend on identity/staging/external boundaries. |
| 9 | S09 | `universal_kb` | High fan-in workflow; keep graph writes and fact promotion disabled. |
| 10 | S10 | `graph_readiness` | Highest integration fan-in; move after foundations/workflows are canonical. |
| 11 | S11 | `rlm` | Depends on graph/retrieval workflow boundaries. |
| 12 | S12 | `llm_minimax`, `miscellaneous`, `entrypoints` | Final cleanup once canonical homes exist and old package can be retired. |

## Cluster Target Map

### S02: Identity

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.identity.__init__` | `research_graph.identity.__init__` | Update exports to canonical names. |
| `arxiv_archive.identity.canonicalization` | `research_graph.identity.canonicalization` | Leaf module; move first. |
| `arxiv_archive.identity.dedup` | `research_graph.identity.dedup` | Leaf module; move first. |

### S02: Staging

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.staging.__init__` | `research_graph.staging.__init__` | Update exports to canonical names. |
| `arxiv_archive.staging.graph_candidates` | `research_graph.staging.graph_candidates` | Depends on identity. |
| `arxiv_archive.staging.import_boundary` | `research_graph.staging.import_boundary` | Depends on identity. |

### S03: Quality

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.quality.__init__` | `research_graph.quality.__init__` | New package marker/export. |
| `arxiv_archive.quality.baselines` | `research_graph.quality.baselines` | Keep thresholds/report contracts intact. |
| `arxiv_archive.quality.maintainability_report` | `research_graph.quality.maintainability_report` | Keep report output stable. |
| `arxiv_archive.quality.riskratchet_adapter` | `research_graph.quality.riskratchet_adapter` | External tool adapter; no network expected. |
| `arxiv_archive.quality.scopes` | `research_graph.quality.scopes` | Leaf config/contract. |
| `arxiv_archive.quality.thresholds` | `research_graph.quality.thresholds` | Leaf config/contract. |

### S04: Repair Chunks

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.bounded_chunk_repair` | `research_graph.repair.bounded_chunk_repair` | Preserve review-only repair contract constraints. |
| `arxiv_archive.candidate_locators` | `research_graph.repair.candidate_locators` | Depends on staging candidates. |
| `arxiv_archive.chunk_baseline_measurement` | `research_graph.repair.chunk_baseline_measurement` | Local measurement only. |
| `arxiv_archive.chunk_import_contract` | `research_graph.repair.chunk_import_contract` | Contract module; keep import eligibility false where applicable. |
| `arxiv_archive.chunk_repair_contract` | `research_graph.repair.chunk_repair_contract` | Contract module; diagnostics must not leak corpus text. |
| `arxiv_archive.chunking_benchmark` | `research_graph.repair.chunking_benchmark` | Keep package-boundary regexes from overmatching `arxiv_archive.chunking`. |

### S05: Extraction and Evaluation

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.dspy_extraction` | `research_graph.evaluation.dspy_extraction` | Do not enable DSPy/provider execution; tests must be local-only. |
| `arxiv_archive.evaluation` | `research_graph.evaluation.metrics` or `research_graph.evaluation.evaluation` | Final module name should be chosen during S05 impact review. |
| `arxiv_archive.extraction_benchmark` | `research_graph.evaluation.extraction_benchmark` | Preserve benchmark fixture contracts. |
| `arxiv_archive.scientific_extraction` | `research_graph.evaluation.scientific_extraction` | Keep deterministic test behavior. |
| `arxiv_archive.scoring` | `research_graph.evaluation.scoring` | Leaf scoring utilities. |

### S06: Retrieval and Embedding

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.embedder` | `research_graph.retrieval.embedder` | No live model/provider calls in tests. |
| `arxiv_archive.hybrid_retrieval` | `research_graph.retrieval.hybrid` | Preserve M003 fixture-level baseline semantics. |
| `arxiv_archive.keyword_extractor` | `research_graph.retrieval.keyword_extractor` | May remain deterministic text utility. |
| `arxiv_archive.summarizer` | `research_graph.retrieval.summarizer` | Guard against provider calls. |

### S07: External Clients and Notifications

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.arxiv_client` | `research_graph.corpus.sources.arxiv_client` | Mock all network in tests. |
| `arxiv_archive.semantic_scholar` | `research_graph.corpus.sources.semantic_scholar` | Mock all network in tests. |
| `arxiv_archive.ladybug_client` | `research_graph.graph.ladybug_client` | Graph side-effect boundary; no writes in migration tests. |
| `arxiv_archive.telegram_sender` | `research_graph.ops.notifications.telegram_sender` | Explicit notification side-effect boundary; no secret logging. |

### S08: Validation Batch Workflow

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.validation_batch_provenance` | `research_graph.workflows.validation.provenance` | Preserve provenance schema. |
| `arxiv_archive.validation_batch_state` | `research_graph.workflows.validation.state` | Preserve state/failure observability. |
| `arxiv_archive.validation_batch_workflow` | `research_graph.workflows.validation.workflow` | Depends on external clients and staging. |
| `arxiv_archive.validation_logging` | `research_graph.workflows.validation.logging` | Keep logs secret-safe. |

### S09: Universal KB Workflows

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.universal_kb_contracts` | `research_graph.workflows.universal_kb.contracts` | Keep production-write/fact-promotion flags false unless separately authorized. |
| `arxiv_archive.universal_kb_queue` | `research_graph.workflows.universal_kb.queue` | Queue contracts and local tests only. |
| `arxiv_archive.universal_kb_rehearsal` | `research_graph.workflows.universal_kb.rehearsal` | Local rehearsal artifacts only. |
| `arxiv_archive.universal_kb_review_assistance` | `research_graph.workflows.universal_kb.review_assistance` | Review assistance, no fact promotion. |
| `arxiv_archive.universal_kb_sidecar_boundary` | `research_graph.workflows.universal_kb.sidecar_boundary` | Boundary contracts. |
| `arxiv_archive.universal_kb_smoke` | `research_graph.workflows.universal_kb.smoke` | Smoke tests local-only. |
| `arxiv_archive.universal_kb_substrate_rehearsal` | `research_graph.workflows.universal_kb.substrate_rehearsal` | Local rehearsal only. |

### S10: Graph Readiness Pipeline

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.graph_readiness` | `research_graph.graph.readiness.core` | Core readiness contracts. |
| `arxiv_archive.graph_readiness_export` | `research_graph.graph.readiness.export` | No unauthorized writes. |
| `arxiv_archive.graph_readiness_extraction_gate` | `research_graph.graph.readiness.extraction_gate` | Depends on evaluation. |
| `arxiv_archive.graph_readiness_manifest` | `research_graph.graph.readiness.manifest` | Must run review post-check before manifest synthesis. |
| `arxiv_archive.graph_readiness_persistence` | `research_graph.graph.readiness.persistence` | Graph-write boundary; tests no-write. |
| `arxiv_archive.graph_readiness_retrieval_validation` | `research_graph.graph.readiness.retrieval_validation` | Depends on retrieval. |
| `arxiv_archive.graph_readiness_review` | `research_graph.graph.readiness.review` | Preserve completed-review checks. |

### S11: RLM Workflow

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.rlm_graph_traversal` | `research_graph.workflows.rlm.graph_traversal` | Depends on graph/retrieval. |
| `arxiv_archive.rlm_workflow` | `research_graph.workflows.rlm.workflow` | No live provider or optimizer calls. |

### S12: LLM MiniMax, Miscellaneous, and Entrypoints

| Old module | Target module | Notes |
|---|---|---|
| `arxiv_archive.llm.__init__` | `research_graph.llm.__init__` | Fold exports into existing canonical LLM package. |
| `arxiv_archive.minimax_structured` | `research_graph.llm.minimax_structured` | Follow MiniMax safe-helper constraints. |
| `arxiv_archive.minimax_usage` | `research_graph.llm.minimax_usage` | Usage checks only; no secret logging. |
| `arxiv_archive.analytics` | `research_graph.evaluation.analytics` or `research_graph.ops.analytics` | Decide during S12 impact review based on callers. |
| `arxiv_archive.artifacts.__init__` | Archive-only or fold into `research_graph.papers.artifacts` | Existing artifact modules already moved; likely archive-only. |
| `arxiv_archive.evidence` | `research_graph.papers.evidence` or `research_graph.graph.evidence` | Existing `research_graph.papers.evidence` exists; inspect impact before final destination. |
| `arxiv_archive.import_boundary_rehearsal` | `research_graph.workflows.import_boundary_rehearsal` | Rehearsal workflow. |
| `arxiv_archive.models_registry` | `research_graph.llm.models_registry` or `research_graph.ops.models_registry` | Decide by caller intent. |
| `arxiv_archive.reviewer_packet_prototype` | `research_graph.workflows.review_packet_prototype` | Workflow artifact helper. |
| `arxiv_archive.thirty_paper_deviation_scan` | `research_graph.corpus.sources.thirty_paper_deviation_scan` | Adjacent to source-scan helper already moved. |
| `arxiv_archive.__main__` | Archive-only or `research_graph.cli.__main__` | Final CLI decision in S12. |
| `arxiv_archive.cli` | `research_graph.cli` | Update entrypoint config if needed. |
| `arxiv_archive.__init__` | Remove/archive old package marker when empty | No permanent shim. |

## Cross-cutting Rules

- Do not introduce permanent `arxiv_archive` compatibility shims.
- Every moved file must be archived under a new `archive/package-rename-waves/wave-XX/` directory with a manifest row and `Formerly:` breadcrumb.
- Update tests and callers atomically inside each slice.
- Use package-boundary old-import regexes; do not match unrelated names like `arxiv_archive.chunking_benchmark` when checking `arxiv_archive.chunking`.
- Keep all tests local-only: no MiniMax, GLM, DSPy provider calls, arXiv PDF downloads, Marker calls, graph writes, fact promotion, or production imports.
- Before editing any function/class/method, run GitNexus impact for the symbol being changed.
- Before a local commit, run `gitnexus_detect_changes()` and stage only explicit pathsets.
