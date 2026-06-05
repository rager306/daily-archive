# M033 S04 QuantMind Implemented vs Vision Map

## Implemented / usable patterns

- **configs** — `implemented`: BaseFlowCfg, BaseInput, PaperFlowCfg, PaperInput variants exist.
- **preprocess.fetch** — `implemented`: Fetch layer returns bytes/metadata; can use network for arXiv/HTTP, local file reads for local input.
- **preprocess.format** — `implemented-partial`: PDF uses PyMuPDF plain text, HTML uses trafilatura; high-fidelity PDF/markdown engines are future/optional.
- **paper_flow** — `implemented-runtime-requires-LLM`: Fetches/formats input and calls OpenAI Agents SDK Agent(output_type=Paper).
- **batch_run** — `implemented-stateless`: Bounded concurrency over flow inputs; memory= is intentionally forbidden in MVP.
- **magic resolver** — `implemented-runtime-requires-LLM`: Introspects flow signature and calls OpenAI Agents SDK to resolve typed input/cfg.
- **BaseKnowledge provenance** — `implemented-pattern`: SourceRef, ExtractionRef, Citation, as_of, confidence, tags, content_hash.
- **TreeKnowledge** — `implemented-pattern`: Hierarchical node graph with root, children, citations, summary/content; PageIndex-style navigation pattern.
- **Paper and PaperKnowledgeCard** — `implemented-pattern`: Paper is TreeKnowledge; PaperKnowledgeCard is FlattenKnowledge summary card linked by paper_id.

## Not ready / aspirational / placeholder

- **GraphKnowledge** — `placeholder-not-implemented`: Subclassing raises NotImplementedError; graph edges are future design intent.
- **storage layer** — `missing-from-package`: Docs may describe storage, but current package tree has no storage implementation.
- **retrieval API / RAG runtime** — `missing-from-package`: README mentions RAG/DeepResearch/Data MCP as vision, not current implemented package layer.
- **memory / mind** — `placeholder-or-roadmap`: memory accepted as opaque placeholder; no production memory/store layer.
- **production semantic KG** — `not-production-ready`: No production KG layer suitable for daily-archive adoption.
- **embedding docs** — `stale-doc-risk`: Useful conceptually, but not implementation proof.

## Quality and documentation risks

- README/docs vision outpaces current package implementation.
- Version mismatch: pyproject 0.2.0 versus __init__ 0.0.1.
- Python mismatch: README badge 3.8+ versus pyproject >=3.10.
- Some BaseFlowCfg fields are not fully enforced in current runner path.
- Core extraction is LLM-agent based, not deterministic non-LLM indexing.

## Classification

- quant-mind as architecture pattern source: `recommended`
- quant-mind as M033 runtime dependency: `not_recommended`
- quant-mind as production RAG/KB: `not_ready`
- quant-mind as graph platform: `not_ready`

## Safety flags

- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `production_import_attempted=false`
- `import_eligible=false`
