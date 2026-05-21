# M019 S02 research-agent profile index

## Profile status

| Target | Profile | Confidence | Most relevant daily-archive pattern | Main non-goal |
|---|---|---:|---|---|
| GPT Researcher | `profiles/gpt-researcher-profile.md` | high | Planner/executor/synthesizer separation, source-first provenance, bounded retrievers | Treating generated reports as KG truth |
| AI-Researcher | `profiles/ai-researcher-profile.md` | medium-high | Stage-separated agents, source-map-first research, explicit completion states | Autonomous code generation and concept-to-publication automation |
| The AI Scientist | `profiles/ai-scientist-profile.md` | high | Phase gates, source maps, failure observability, sandboxing lessons | Executing LLM-written code or autonomous paper generation |
| prismAId | `profiles/prismaid-profile.md` | high | Protocol-as-config, source ledger, screening/conversion/review gates | Copying AGPL code or treating AI extraction as peer review |

## Cross-profile early observations

1. **Most aligned with daily-archive:** prismAId, because it is protocol-bound and systematic-review oriented rather than autonomous-scientist oriented.
2. **Most useful orchestration pattern:** GPT Researcher, because its planner/executor/publisher split maps well to source acquisition -> summarization -> synthesis.
3. **Most useful cautionary example:** The AI Scientist, because it explicitly warns about executing LLM-written code, web access, process spawning, and containerization.
4. **Most risky autonomy model:** AI-Researcher and The AI Scientist, because both target broad autonomous scientific innovation/paper generation.

## Known gaps for S03

- AI-Researcher license is still unclear from S01/S02 evidence.
- The profiles use repo/docs/paper evidence, not local cloned source inspection.
- S03 should avoid recommending dependency adoption; this spike is pattern-level only.
- S03 should convert profile observations into a small set of daily-archive design recommendations for KG candidate locators/chunk-span provenance.

## Safety

No third-party code copied. No raw corpus content, embeddings, vectors, secrets, model payloads, or external write actions were persisted.
