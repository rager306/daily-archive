# GPT Researcher profile for daily-archive

## Architecture and workflow

GPT Researcher is an open-source deep-research agent positioned for web and local-document research with generated reports and citations. Its core workflow is planner/executor/publisher:

1. Generate an outline of research questions from the user query.
2. Run crawler/research agents for each question.
3. Scrape or retrieve candidate sources.
4. Filter and summarize relevant resources while tracking sources.
5. Aggregate summaries into a final report.

Evidence: README architecture section describes planner and execution agents, crawler-based gathering, summarization, source tracking, filtering, aggregation, and report generation. The “How we built GPT Researcher” article describes a deterministic plan-and-solve workflow that decomposes research into a finite set of subtasks, executes each outline item, tracks/summarizes sources, then generates a report.

Sources:

- https://raw.githubusercontent.com/assafelovic/gpt-researcher/main/README.md
- https://docs.gptr.dev/blog/building-gpt-researcher
- https://github.com/assafelovic/gpt-researcher

## Source acquisition

GPT Researcher supports multiple acquisition modes:

- Web search via retrievers. Tavily is the documented default; other supported retrievers include Bing, Google, SearchApi, SerpAPI, Serper, Searx, DuckDuckGo, arXiv, Exa, and PubMedCentral.
- Custom retrievers via `RETRIEVER=custom`, where the endpoint returns source objects with URLs and raw content.
- Static source lists via `source_urls`; `complement_source_urls=False` restricts research to the provided URLs, while `True` allows additional discovered sources.
- Local documents via `DOC_PATH` and `report_source="local"`; supported formats include PDF, text, CSV, Excel, Markdown, PowerPoint, and Word.
- Hybrid web/local research and MCP-enabled retrieval, including hybrid `tavily,mcp` configurations.

For daily-archive, the most reusable acquisition pattern is not “use GPT Researcher as-is,” but the acquisition contract: make retrievers explicit, record source URLs before synthesis, support allowlisted/static source sets, and separate retrieved raw content from derived summaries.

Sources:

- https://docs.gptr.dev/docs/gpt-researcher/search-engines
- https://docs.gptr.dev/docs/gpt-researcher/context/tailored-research
- https://docs.gptr.dev/docs/gpt-researcher/retrievers/mcp-configs
- https://raw.githubusercontent.com/assafelovic/gpt-researcher/main/README.md

## Provenance and citations

Provenance is a first-class concept in GPT Researcher:

- The README claims generated research reports include citations and describes “summarize and source-track each resource.”
- The PIP package docs expose getters for source URLs, research context, research costs, research images, and research sources.
- The logging docs include event types such as `added_source_url`, `scraping_urls`, `scraping_content`, and `research_step_finalized`, giving a reconstructable trail of source discovery and processing.
- The original architecture article says the report prompt should write source URLs at the end and only use the provided aggregated information.

For daily-archive, this maps directly to a source-map/provenance model: every synthesized claim should retain a link to source URL(s), acquisition method, retrieval timestamp, and derived-summary boundary. Generated prose should never be treated as graph fact unless the underlying source evidence is independently represented.

Sources:

- https://raw.githubusercontent.com/assafelovic/gpt-researcher/main/README.md
- https://docs.gptr.dev/docs/gpt-researcher/gptr/pip-package
- https://docs.gptr.dev/docs/gpt-researcher/handling-logs/all-about-logs
- https://docs.gptr.dev/blog/building-gpt-researcher

## Review and quality gates

GPT Researcher’s quality controls are a mix of architectural gates, observability, and project tests:

- Research quality gate: decompose into subquestions, gather multiple sources, summarize only relevant content, aggregate only after subquery work completes.
- Bias/factuality strategy: the project argues that using many relevant sources plus summarization of retrieved content reduces hallucination and bias, while acknowledging minor hallucinations can still occur.
- Execution observability: report logs capture timestamped events for planning, subqueries, added source URLs, scraping, context windows, draft/report writing, and completion.
- Engineering quality gate: docs describe automated tests run with pytest in Docker and GitHub Actions, requiring provider secrets for integration-like test execution.

For daily-archive, the stronger quality gate should be stricter than GPT Researcher’s default report generation: validate source count, source diversity, parse success, citation coverage, retrieval errors, and claim-to-source linkage before inserting facts into a knowledge graph.

Sources:

- https://docs.gptr.dev/blog/building-gpt-researcher
- https://docs.gptr.dev/docs/gpt-researcher/handling-logs/all-about-logs
- https://docs.gptr.dev/docs/gpt-researcher/gptr/automated-tests
- https://raw.githubusercontent.com/assafelovic/gpt-researcher/main/README.md

## Autonomy boundaries

GPT Researcher is autonomous within configured boundaries, not unconstrained:

- The planner creates a finite set of subqueries/questions rather than an unbounded AutoGPT-style loop.
- The retriever set is configured by environment variables such as `RETRIEVER`; each provider has its own API key requirements and usage limits.
- Static-source mode can prevent expansion beyond provided URLs with `complement_source_urls=False`.
- MCP support requires explicit MCP server configuration and then uses a two-stage LLM process to select and call available tools.
- Local-document research is scoped by `DOC_PATH`.

For daily-archive, safe autonomy boundaries should be explicit: allowed retrievers, allowed domains, maximum source count, maximum recursion/depth, maximum token/cost budget, citation-required synthesis, and no automatic KG writes from generated reports.

Sources:

- https://docs.gptr.dev/blog/building-gpt-researcher
- https://docs.gptr.dev/docs/gpt-researcher/search-engines
- https://docs.gptr.dev/docs/gpt-researcher/context/tailored-research
- https://docs.gptr.dev/docs/gpt-researcher/retrievers/mcp-configs

## Failure modes

Documented and inferred-from-docs failure modes relevant to daily-archive:

- Missing or invalid API keys for LLM/search providers; setup requires keys such as OpenAI and Tavily in normal usage.
- Model access failures, e.g. documented model permission errors.
- Provider limits and cost variability; search engine docs warn each engine has its own API key requirements and usage limits, and PIP docs expose research cost tracking.
- Scraping failures; troubleshooting says some Selenium-scraped sites fail and suggests restarting/retrying.
- Browser/driver incompatibility; troubleshooting documents Chrome/chromedriver version issues.
- Native dependency failures for export paths; troubleshooting mentions WeasyPrint-related library errors.
- Hallucination risk remains; the architecture article says results had minor hallucinations in some samples despite source-grounded prompting.
- Raw-context risk: PIP docs expose `get_research_context()` containing retrieved information and corresponding content, which daily-archive should not persist wholesale if the project policy forbids raw corpus retention.

Sources:

- https://docs.gptr.dev/docs/gpt-researcher/getting-started
- https://docs.gptr.dev/docs/gpt-researcher/search-engines
- https://docs.gptr.dev/docs/gpt-researcher/gptr/pip-package
- https://docs.gptr.dev/docs/gpt-researcher/gptr/troubleshooting
- https://docs.gptr.dev/blog/building-gpt-researcher

## Reusable patterns for daily-archive

- **Planner/executor/synthesizer separation:** keep query planning, source acquisition, evidence normalization, and final synthesis as distinct phases with persisted phase state.
- **Finite research plans:** convert broad questions into bounded subqueries; avoid open-ended agent loops.
- **Source-first provenance:** record discovered URLs and retrieval metadata before summarization; preserve citation coverage through synthesis.
- **Static source mode:** support allowlisted URLs for tasks where authoritative sources are already known.
- **Hybrid retrieval:** combine arXiv/domain-specific retrievers with web or MCP retrievers only when explicitly configured.
- **Event log surface:** emulate GPT Researcher’s event model with machine-readable events for `planning`, `subqueries`, `source_added`, `fetch_started`, `fetch_failed`, `summary_created`, `quality_gate_passed`, and `report_written`.
- **Cost and budget tracking:** expose token/API cost estimates per run.
- **Generated prose quarantine:** store generated reports as derived artifacts, not as primary KG facts; promote only source-backed structured claims after validation.
- **Failure transparency:** persist scrape failures, excluded sources, provider errors, and low-confidence claims instead of silently dropping them.

## Non-goals and safety risks

- **Do not copy GPT Researcher code.** The repository is Apache-2.0 licensed, but this profile only extracts architectural patterns and source-backed observations.
- **Do not persist raw third-party corpus content.** GPT Researcher can expose retrieved context, but daily-archive should persist source metadata, derived summaries, and claim evidence only.
- **Do not treat generated reports as truth.** GPT Researcher optimizes for source-grounded reports, but residual hallucination risk remains.
- **Do not give agents unrestricted web/MCP access.** Retriever, MCP, local document, domain, and source-list boundaries should be explicit.
- **Do not import export/document-generation complexity unless needed.** PDF/Word export introduces native dependency failure modes unrelated to daily-archive’s core KG/research pipeline.
- **Do not rely on API-key-dependent tests as the only quality gate.** Provider-backed tests are useful, but daily-archive also needs deterministic fixture tests for acquisition, provenance, citation coverage, and failure logging.
- **Do not allow secret leakage.** GPT Researcher workflows require provider keys; daily-archive logs and artifacts must record key names/config status only, never secret values.

Sources:

- https://raw.githubusercontent.com/assafelovic/gpt-researcher/main/LICENSE
- https://docs.gptr.dev/docs/gpt-researcher/gptr/pip-package
- https://docs.gptr.dev/blog/building-gpt-researcher
- https://docs.gptr.dev/docs/gpt-researcher/retrievers/mcp-configs
