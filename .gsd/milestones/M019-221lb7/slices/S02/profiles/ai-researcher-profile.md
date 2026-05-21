# AI-Researcher profile for daily-archive

## Architecture and workflow

AI-Researcher is positioned as an end-to-end autonomous scientific discovery system: literature review, hypothesis/idea generation, algorithm design, implementation, validation/refinement, result analysis, and manuscript creation. Its README explicitly describes “full autonomy” and “from concept to publication,” while the paper frames the system as a multi-agent architecture for literature exploration, idea generation, algorithm implementation, experimental validation, and scholarly publication.

Sources:

- https://raw.githubusercontent.com/HKUDS/AI-Researcher/main/README.md
- https://arxiv.org/abs/2505.18705
- https://arxiv.org/html/2505.18705v1

The paper’s appendix decomposes the system into specialized agents:

- **Knowledge Acquisition Agent:** selects relevant reference codebases from search results.
- **Resource Analyst:** includes Paper Analyst, Code Analyst, and Plan Agent.
- **Code Agent:** builds a self-contained implementation from the plan and reference materials.
- **Advisor Agent:** includes Judge Agent, Code Review Agent, and Experiment Analysis Agent.
- **Automated Documentation Agent:** turns research artifacts into manuscript-style documentation.

For daily-archive, the useful architectural idea is not full autonomy, but staged evidence flow: source discovery -> structured extraction -> implementation/analysis plan -> review gate -> summarized artifact.

## Source acquisition

AI-Researcher acquires sources through a reference-driven workflow. Its README describes two input levels: users either provide a detailed research idea or provide reference papers and ask the system to generate and implement an idea from them. The appendix describes a Knowledge Acquisition Agent that reviews search results and selects 5 to 8 relevant GitHub repositories based on recency, stars, README quality, code structure, Python/PyTorch preference, and local runnability.

Sources:

- https://raw.githubusercontent.com/HKUDS/AI-Researcher/main/README.md
- https://arxiv.org/html/2505.18705v1

This source acquisition model is risky if copied directly: it clones and inspects external repositories as implementation references. For daily-archive, reuse only the **selection criteria and source-map habit**, not raw repository ingestion or code copying.

## Provenance and citations

AI-Researcher’s paper emphasizes mapping academic concepts to both paper evidence and code implementations. The Paper Analyst extracts definitions, formulas, and theory from papers; the Code Analyst maps those concepts to implementation details; the Code Agent is instructed to document origins and modifications of adapted ideas.

Source:

- https://arxiv.org/html/2505.18705v1

Reusable pattern for daily-archive:

- Keep a compact source map per research run.
- Record source URL, source type, confidence, and evidence text.
- Separate extracted claims from raw corpus content.
- Preserve citation URLs in final summaries.
- Never treat generated synthesis as source-of-truth without traceable upstream sources.

This aligns with daily-archive’s need for auditable arXiv/archive/KG content.

## Review and quality gates

AI-Researcher evaluates output with multiple gates:

- **Implementation completeness:** whether the agent produces executable code within budget, using explicit `case_resolved` / `case_not_resolved` termination.
- **Implementation correctness:** Advisor and Judge agents inspect conceptual fidelity and score quality.
- **Paper-quality review:** specialized review agents compare AI-generated papers against human papers using dimensions like novelty, methodological rigor, and empirical validation.
- **Multi-model judging:** the paper reports evaluation using several LLM judges to reduce single-model bias.

Source:

- https://arxiv.org/html/2505.18705v1

For daily-archive, the reusable gate is narrower:

1. **Source gate:** every claim has a URL-backed source.
2. **Extraction gate:** no raw third-party corpus is persisted unless allowed.
3. **Provenance gate:** generated artifacts include citations and confidence.
4. **Quality gate:** summaries distinguish observed facts from agent interpretation.
5. **Boundary gate:** no autonomous code adoption, publication, or external action.

## Autonomy boundaries

AI-Researcher explicitly targets minimal-human-intervention research automation, including ideation, implementation, experiments, and paper generation. That autonomy is too broad for daily-archive.

Sources:

- https://raw.githubusercontent.com/HKUDS/AI-Researcher/main/README.md
- https://arxiv.org/abs/2505.18705

Recommended boundary for daily-archive:

- Allow autonomous **source discovery, summarization, citation extraction, and KG-ready profile drafting**.
- Require human or project policy approval for **new dependencies, copied algorithms, benchmark claims, publication-style assertions, and any external write action**.
- Treat LLM review as a signal, not an authority.
- Prefer “assistive researcher” behavior over “autonomous scientist” behavior.

## Failure modes

The AI-Researcher paper identifies several relevant failure modes:

- **Implementation fidelity drift:** models may know the right approach in isolation but fail to preserve full requirements across multi-turn workflows.
- **Premature completion:** agents can claim success with partial or conceptually incomplete implementations.
- **Tensor/data/debug failures:** reported failures include tensor dimension conflicts, datatype mismatches, NaN losses, and training instability.
- **Oversimplification:** one example describes a model claiming a Diffusion Transformer while actually producing a standard Vision Transformer without diffusion components.
- **Memory compression loss:** the paper states the system lacks a dedicated external memory system and relies heavily on context-window summaries, causing fine-grained details to be lost across long workflows.
- **Evaluator bias:** LLM reviewers may diverge substantially and may overweight presentation quality over substantive contribution.

Source:

- https://arxiv.org/html/2505.18705v1

For daily-archive, the most important analogs are citation drift, source/claim mismatch, overconfident summaries, and loss of fine-grained provenance during artifact compression.

## Reusable patterns for daily-archive

- **Source-map-first research:** use a structured JSON source map before writing profiles.
- **Stage-separated agents or phases:** acquisition, extraction, synthesis, and review should remain separate so errors are easier to localize.
- **Atomic claim checking:** break profile claims into small units that can be tied to URLs.
- **Explicit completion states:** distinguish “found,” “not found,” “partial,” and “blocked” rather than silently producing weak profiles.
- **Review before persistence:** run a quality/provenance gate before writing KG artifacts or summary files.
- **No raw corpus retention:** persist only metadata, citations, concise evidence snippets, and derived summaries.
- **Memory externalization:** keep durable run evidence and source maps instead of relying on conversational context.
- **Evaluator diversity where warranted:** for high-impact classification or quality judgments, compare multiple review passes or criteria instead of a single LLM judgment.

## Non-goals and safety risks

Non-goals for daily-archive:

- Do not implement AI-Researcher’s autonomous code-generation pipeline.
- Do not copy third-party code, prompts, benchmark data, or raw paper corpus content.
- Do not adopt its “concept to publication” autonomy as a daily-archive pattern.
- Do not let generated manuscripts or summaries become authoritative without source-backed provenance.
- Do not clone arbitrary repositories or execute external code as part of routine archive profiling.

Safety risks:

- **Scientific overclaiming:** AI-Researcher’s framing encourages publication-like outputs; daily-archive should keep profiles evidential and bounded.
- **License/provenance ambiguity:** the S01 source map notes no root LICENSE was found via fetched root license URL; avoid code reuse unless licensing is separately verified.
- **Supply-chain risk:** repository cloning, Docker images, API keys, and executable research environments introduce avoidable risk for an archive/KG workflow.
- **Citation laundering:** generated profiles can make weak claims look source-backed unless every claim maps to an actual URL.
- **Evaluator bias:** LLM review scores are not equivalent to expert validation.

## Source URLs

- Repository: https://github.com/HKUDS/AI-Researcher
- README: https://raw.githubusercontent.com/HKUDS/AI-Researcher/main/README.md
- arXiv abstract: https://arxiv.org/abs/2505.18705
- arXiv HTML paper: https://arxiv.org/html/2505.18705v1
- Project docs URL listed by README, returned 404 during inspection: https://autoresearcher.github.io/docs
