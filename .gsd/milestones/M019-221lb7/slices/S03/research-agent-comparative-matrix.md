# M019 research-agent comparative matrix

## Final recommendation

`Use protocol-bound review patterns, not autonomous scientist patterns.`

For daily-archive, the strongest pattern source is **prismAId** because it frames research as a protocol-bound workflow with explicit stages, configuration, screening, conversion, review, and audit outputs. **GPT Researcher** is useful for bounded orchestration and source-first provenance patterns. **AI-Researcher** and **The AI Scientist** are useful mostly as cautionary examples: they show why daily-archive should not adopt autonomous code execution, concept-to-publication loops, or generated manuscript authority.

## Comparative matrix

| Dimension | GPT Researcher | AI-Researcher | The AI Scientist | prismAId | daily-archive conclusion |
|---|---|---|---|---|---|
| Primary goal | Deep research reports from web/local sources | Autonomous scientific innovation from idea/reference papers to manuscript | Fully automated open-ended ML scientific discovery | Systematic, protocol-based literature review | Prefer protocol-bound evidence workflows over autonomous scientist loops |
| Architecture | Planner -> execution/crawler agents -> summarizer/publisher | Knowledge acquisition, resource analyst, code agent, advisor, documentation agent | Template -> idea generation -> novelty check -> experiments -> paper -> review | Search -> Screen -> Download -> Convert -> Review | Use stage-separated phases with persisted artifacts |
| Source acquisition | Configured retrievers, source URLs, local docs, MCP | Reference papers plus selected GitHub repos | Templates plus Semantic Scholar/OpenAlex and generated experiment artifacts | URL/Zotero inputs, DOI/Crossref/Unpaywall fallback | Build source ledgers with acquisition status/error reasons |
| Provenance | Source URLs and source tracking through report synthesis | Paper/code concept mapping and generated documentation | Citations and generated run artifacts | Metadata, filenames, CSV/JSON outputs, protocol configs | Require source map before synthesis and claim-to-source traceability |
| Review gates | Multi-source aggregation, logs, tests, report citations | Advisor/Judge/review agents and multi-model evaluation | Novelty checks, baseline runs, automated reviewer | Protocol config, schema, small-batch validation, conversion/screening QA | Combine deterministic guards, review queues, and explicit failures |
| Autonomy | Bounded by retriever/source config, but generates reports | Broad autonomy from concept to publication | Very broad; executes LLM-written code and writes papers | Protocol-bound automation under user config | Keep agents assistive, never authoritative |
| Failure modes | Scrape/API failures, hallucination, raw-context risk, export deps | fidelity drift, premature completion, evaluator bias, memory loss | self-modification, runaway execution, bad plots, unfair baselines | acquisition/conversion errors, LLM nondeterminism, screening false positives | Persist failure states and review uncertain outputs |
| License/reuse | Apache-2.0 | License unclear from S01/S02 root fetch | Custom Responsible-AI-style source license | AGPL-3.0 | Reuse patterns only; no code copying in this spike |
| Fit for KG candidate locators | Medium-high for orchestration/provenance | Low-medium; useful gates but too autonomous | Low for direct adoption; high for safety lessons | High for protocol/review/candidate extraction workflow | Anchor next KG milestone on prismAId-style protocol plus GPT Researcher-style source tracking |

## Concrete reusable patterns

### 1. Protocol-as-config

Borrow from prismAId: configure extraction/review protocols outside code. For candidate locators, define:

```text
candidate_type
allowed evidence fields
required source span fields
allowed uncertainty labels
failsafe values
review queue reasons
```

This should become a versioned daily-archive artifact before positive KG import resumes.

### 2. Source ledger before synthesis

Borrow from prismAId and GPT Researcher: every candidate source should have a ledger row before any generated summary:

```text
paper_id
source_url_or_path
source_type
acquisition_method
conversion_method
conversion_status
error_reason
hash_or_size_metadata
raw_text_persisted=false
```

### 3. Bounded planner/executor/synthesizer split

Borrow from GPT Researcher but narrow it:

```text
planner: decide which evidence fields/queries are needed
executor: retrieve/check source artifacts only
synthesizer: create candidate locator summaries
reviewer: validate citation/span coverage before persistence
```

No phase should silently skip failures or write KG facts.

### 4. Review queues and small-batch rollout

Borrow from prismAId:

- run one-paper fixture first;
- then a small reviewed batch;
- only then a larger validation batch;
- route uncertain/missing/contradictory evidence to review queue;
- keep excluded/skipped candidates visible with reasons.

### 5. Failure-state observability

Borrow across all systems:

```text
source_fetch_failed
conversion_low_quality
span_missing
locator_ambiguous
evidence_conflict
model_output_invalid
review_required
```

These should be structured diagnostics, not prose-only notes.

## Explicit non-goals

Daily-archive should not adopt:

- autonomous code generation from AI-Researcher;
- autonomous experiment execution from The AI Scientist;
- autonomous paper/manuscript generation as authority;
- automatic external writes, submissions, GitHub actions, or publication steps;
- generated research reports as KG facts;
- third-party code copying from AGPL/custom-license projects;
- raw corpus persistence in GSD artifacts;
- hidden chain-of-thought logs or model payload dumps;
- broad web/MCP access without source/domain/budget gates.

## Recommended next milestone

Return to Scientific KG readiness with:

```text
KG Candidate Locator and Chunk-Span Provenance Protocol
```

Suggested slices:

1. **Protocol contract:** define candidate locator schema, source span fields, uncertainty labels, and review queue reasons.
2. **One-paper locator fixture:** produce candidate locators over one known paper with exact source-span references and no KG import.
3. **Small-batch locator rehearsal:** run over a bounded reviewed batch, measure missing/ambiguous/conflicting spans, and keep import disabled.
4. **Independent semantic review:** verify whether locators are meaningful enough to consider a future positive import gate.

## Decision

Validate R047 as complete. The comparison provides enough evidence to inform the next KG/provenance milestone. It does not justify adopting any research-agent code or enabling autonomous behavior.

## Sources

Profiles:

- `.gsd/milestones/M019-221lb7/slices/S02/profiles/gpt-researcher-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/ai-researcher-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/ai-scientist-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/prismaid-profile.md`

Primary URLs include:

- `https://github.com/assafelovic/gpt-researcher`
- `https://github.com/HKUDS/AI-Researcher`
- `https://github.com/SakanaAI/AI-Scientist`
- `https://github.com/Open-and-Sustainable/prismAId`
- `https://prismaid.review`
