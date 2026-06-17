# Target Package Map: `research_graph`

## Target top-level package

```text
src/research_graph/
```

The distribution may remain `arxiv-daily-archive` temporarily during migration, but the importable package target is `research_graph`.

## Target structure

```text
src/research_graph/
├── __init__.py
├── cli.py
├── corpus/
│   ├── __init__.py
│   ├── ingestion/
│   │   ├── fetchers.py
│   │   ├── loader.py
│   │   └── logging.py
│   ├── parsing/
│   │   ├── normalization.py
│   │   ├── parser.py
│   │   └── structure.py
│   └── sources/
│       ├── arxiv_client.py
│       ├── semantic_scholar.py
│       └── source_asset_manifest.py
├── papers/
│   ├── __init__.py
│   ├── artifacts/
│   │   ├── models.py
│   │   ├── metrics.py
│   │   ├── minimax_boundary.py
│   │   ├── reducer.py
│   │   ├── worker.py
│   │   └── batch_validation.py
│   ├── assets.py
│   ├── evidence.py
│   ├── links.py
│   ├── page_index.py
│   └── retrieval_tables.py
├── graph/
│   ├── __init__.py
│   ├── client.py
│   ├── readiness.py
│   ├── review.py
│   ├── export.py
│   ├── manifest.py
│   ├── persistence.py
│   ├── retrieval_validation.py
│   └── traversal.py
├── staging/
│   ├── __init__.py
│   ├── candidates.py
│   └── import_boundary.py
├── identity/
│   ├── __init__.py
│   ├── canonicalization.py
│   └── dedup.py
├── llm/
│   ├── __init__.py
│   ├── provider_config.py
│   ├── minimax_structured.py
│   └── minimax_usage.py
├── evaluation/
│   ├── __init__.py
│   ├── benchmark.py
│   ├── scoring.py
│   ├── evidence.py
│   ├── extraction.py
│   └── quality.py
├── repair/
│   ├── __init__.py
│   ├── bounded_chunk_repair.py
│   ├── chunk_import_contract.py
│   └── chunk_repair_contract.py
└── workflows/
    ├── __init__.py
    ├── rlm.py
    ├── validation_batch.py
    └── universal_kb.py
```

## Mapping from current `arxiv_archive`

### Corpus

```text
arxiv_archive.ingestion.*        -> research_graph.corpus.ingestion.*
arxiv_archive.parsing.*          -> research_graph.corpus.parsing.*
arxiv_archive.arxiv_client       -> research_graph.corpus.sources.arxiv_client
arxiv_archive.semantic_scholar   -> research_graph.corpus.sources.semantic_scholar
arxiv_archive.source_asset_manifest -> research_graph.corpus.sources.source_asset_manifest
```

### Papers

```text
arxiv_archive.artifacts.metrics          -> research_graph.papers.artifacts.metrics
arxiv_archive.artifacts.minimax_boundary -> research_graph.papers.artifacts.minimax_boundary
arxiv_archive.artifacts.reducer          -> research_graph.papers.artifacts.reducer
arxiv_archive.artifacts.assets           -> research_graph.papers.assets
arxiv_archive.artifacts.evidence_bridge  -> research_graph.papers.evidence
arxiv_archive.article_artifacts          -> research_graph.papers.artifacts.models
arxiv_archive.article_artifact_worker    -> research_graph.papers.artifacts.worker
arxiv_archive.article_batch_validation   -> research_graph.papers.artifacts.batch_validation
arxiv_archive.article_links_dedup        -> research_graph.papers.links
arxiv_archive.article_page_index         -> research_graph.papers.page_index
arxiv_archive.article_retrieval_tables   -> research_graph.papers.retrieval_tables
```

### Indexing

Current `arxiv_archive.indexing` is a transitional package. Most paper-specific indexing should land under `research_graph.papers`. Generic navigation helpers can either remain under `research_graph.papers.page_index` or become `research_graph.corpus.indexing` if used outside papers.

### Graph

```text
arxiv_archive.graph_readiness                      -> research_graph.graph.readiness
arxiv_archive.graph_readiness_review               -> research_graph.graph.review
arxiv_archive.graph_readiness_export               -> research_graph.graph.export
arxiv_archive.graph_readiness_manifest             -> research_graph.graph.manifest
arxiv_archive.graph_readiness_persistence          -> research_graph.graph.persistence
arxiv_archive.graph_readiness_retrieval_validation -> research_graph.graph.retrieval_validation
arxiv_archive.rlm_graph_traversal                  -> research_graph.graph.traversal
arxiv_archive.ladybug_client                       -> research_graph.graph.client
```

### Staging and identity

```text
arxiv_archive.staging.graph_candidates  -> research_graph.staging.candidates
arxiv_archive.staging.import_boundary   -> research_graph.staging.import_boundary
arxiv_archive.identity.*                -> research_graph.identity.*
```

### LLM

```text
arxiv_archive.llm.provider_config -> research_graph.llm.provider_config
arxiv_archive.minimax_structured  -> research_graph.llm.minimax_structured
arxiv_archive.minimax_usage       -> research_graph.llm.minimax_usage
```

### Evaluation and repair

```text
arxiv_archive.extraction_benchmark      -> research_graph.evaluation.benchmark
arxiv_archive.scoring                   -> research_graph.evaluation.scoring
arxiv_archive.evidence                  -> research_graph.evaluation.evidence
arxiv_archive.scientific_extraction     -> research_graph.evaluation.extraction
arxiv_archive.quality.*                 -> research_graph.evaluation.quality.*
arxiv_archive.bounded_chunk_repair      -> research_graph.repair.bounded_chunk_repair
arxiv_archive.chunk_import_contract     -> research_graph.repair.chunk_import_contract
arxiv_archive.chunk_repair_contract     -> research_graph.repair.chunk_repair_contract
```

### Workflows and CLI

```text
arxiv_archive.rlm_workflow                -> research_graph.workflows.rlm
arxiv_archive.validation_batch_state      -> research_graph.workflows.validation_batch
arxiv_archive.validation_batch_workflow   -> research_graph.workflows.validation_batch
arxiv_archive.validation_batch_provenance -> research_graph.workflows.validation_batch
arxiv_archive.universal_kb_queue          -> research_graph.workflows.universal_kb
arxiv_archive.universal_kb_contracts      -> research_graph.workflows.universal_kb
arxiv_archive.cli                         -> research_graph.cli
```

## Breadcrumb rule

Every moved canonical module must contain:

```text
Formerly: src/arxiv_archive/<old_path>.py
```

If a module moves from an existing canonical subpackage, use the exact old file path, for example:

```text
Formerly: src/arxiv_archive/artifacts/metrics.py
```

## Archive rule

Old files are preserved under:

```text
archive/package-rename-waves/wave-XX/src/arxiv_archive/<old_path>.py
```

Do not keep old `arxiv_archive` shims in `src` after a wave is verified unless a deliberate compatibility window is explicitly planned.
