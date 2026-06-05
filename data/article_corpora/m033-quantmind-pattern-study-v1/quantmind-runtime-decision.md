# M033 S04 QuantMind Runtime Decision

## Decision

Do not run live `quantmind.paper_flow`, `resolve_magic_input`, arXiv/HTTP fetches, OpenAI Agents calls, or embedding provider calls in M033/S04. Treat `quant-mind` as a static architecture-pattern source.

## Requirements observed

- Python requirement in `pyproject.toml`: `>=3.10`.
- README badge says Python 3.8+, creating a documentation mismatch.
- Runtime dependencies include `openai`, `openai-agents`, `litellm`, `arxiv`, `httpx`, `pymupdf`, `trafilatura`, and Pydantic.
- Optional `full` extras include `marker-pdf`, `beautifulsoup4`, and `sentence-transformers`.
- `.env.example` documents `OPENAI_API_KEY` and `LLAMA_CLOUD_API_KEY`.
- No Dockerfile/compose runtime is required or present for normal use.

## Why no runtime probe

S04 asks whether quant-mind provides reusable paper-knowledge architecture patterns. Running the live flow would test external model/API and network behavior, require secrets, and blur M033's fail-closed parser research boundary. The useful evidence is in the implemented schemas and layering, especially `TreeKnowledge`, `PaperKnowledgeCard`, `SourceRef`, `Citation`, `ExtractionRef`, and fetch-format-flow separation.

## Safety boundary

- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `production_import_attempted=false`
- `import_eligible=false`
- `model_call_required_for_pattern_study=false`
- `network_required_for_pattern_study=false`
