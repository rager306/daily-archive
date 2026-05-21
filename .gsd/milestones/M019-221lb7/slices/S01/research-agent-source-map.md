# M019 S01 research-agent source map

## Scope

Identify authoritative sources for four target systems before deeper architecture profiling:

```text
GPT Researcher
AI-Researcher
The AI Scientist
prismAId
```

This slice only maps sources. It does not adopt dependencies, copy code, or make architecture recommendations beyond source confidence and caveats.

## Search queries used

```text
GPT Researcher GitHub repository official docs
site:github.com GPT Researcher assafelovic gpt-researcher
AI-Researcher GitHub repository open source research agent
"AI-Researcher" "GitHub" "research agent"
The AI Scientist GitHub repository Sakana AI
"AI-Scientist" "SakanaAI" "GitHub"
prismAId GitHub repository research agent
"prismAId" OR "Prismer" "GitHub" "research"
```

## Source map

| Target | Status | Confidence | Primary repo | License visible | Primary positioning |
|---|---|---:|---|---|---|
| GPT Researcher | found | high | `https://github.com/assafelovic/gpt-researcher` | Apache-2.0 | Web/local deep research agent with citations and reports |
| AI-Researcher | found | high | `https://github.com/HKUDS/AI-Researcher` | root `LICENSE` not found by raw fetch | Autonomous scientific innovation, concept-to-publication automation |
| The AI Scientist | found | high | `https://github.com/SakanaAI/AI-Scientist` | custom Responsible-AI-style source license | Fully automated open-ended scientific discovery and paper generation |
| prismAId | found | high | `https://github.com/Open-and-Sustainable/prismAId` | AGPL-3.0 | Protocol-based systematic literature review toolkit |

## Target notes

### GPT Researcher

Authoritative sources:

- Repository: `https://github.com/assafelovic/gpt-researcher`
- README: `https://raw.githubusercontent.com/assafelovic/gpt-researcher/main/README.md`
- Docs: `https://docs.gptr.dev/docs/gpt-researcher/getting-started`
- License: `https://raw.githubusercontent.com/assafelovic/gpt-researcher/main/LICENSE`

Useful S02 angles:

- planner/execution agent split;
- crawler/resource summarization;
- source tracking;
- parallelized research;
- report generation as non-authoritative synthesis.

### AI-Researcher

Authoritative sources:

- Repository: `https://github.com/HKUDS/AI-Researcher`
- README: `https://raw.githubusercontent.com/HKUDS/AI-Researcher/main/README.md`
- Paper: `https://arxiv.org/abs/2505.18705`
- Docs: `https://autoresearcher.github.io/docs`

Useful S02 angles:

- literature review and idea generation pipeline;
- algorithm design/implementation loop;
- benchmark and dataset claims;
- autonomy risks from concept-to-publication workflow.

### The AI Scientist

Authoritative sources:

- Repository: `https://github.com/SakanaAI/AI-Scientist`
- README: `https://raw.githubusercontent.com/SakanaAI/AI-Scientist/main/README.md`
- Paper: `https://arxiv.org/abs/2408.06292`
- Blog: `https://sakana.ai/ai-scientist/`
- License: `https://raw.githubusercontent.com/SakanaAI/AI-Scientist/main/LICENSE`

Useful S02 angles:

- negative/control example for autonomy boundaries;
- containerization warnings;
- LLM-written code execution risks;
- paper review/generation as something daily-archive should not copy into KG authority.

### prismAId

Authoritative sources:

- Repository: `https://github.com/Open-and-Sustainable/prismAId`
- README: `https://raw.githubusercontent.com/Open-and-Sustainable/prismAId/main/README.md`
- Docs: `https://prismaid.review`
- License: `https://raw.githubusercontent.com/Open-and-Sustainable/prismAId/main/LICENSE`
- Zenodo DOI: `https://doi.org/10.5281/zenodo.11210796`
- JOSS DOI: `https://doi.org/10.21105/joss.07616`

Useful S02 angles:

- systematic review workflow: Search -> Screen -> Download -> Convert -> Review;
- protocol-first configuration;
- screening and reproducibility;
- structured CSV/JSON output.

## Disambiguation

Search results also returned `Prismer-AI/Prismer`, but the user-specified `prismAId` and official documentation at `prismaid.review` identify `Open-and-Sustainable/prismAId` as the relevant target. `Prismer-AI/Prismer` is not part of the S02 target set unless the user later asks to include it.

## Safety

No implementation files changed. No third-party code was copied. No raw paper/chunk/PDF text, secrets, embeddings, vectors, or model payloads were persisted.
