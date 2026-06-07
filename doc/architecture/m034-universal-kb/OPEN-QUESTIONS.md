# M034 Open Questions

Open questions are not accepted decisions. They must not be treated as authorization.

| Question | Owner | Needed By | Blocking? | Notes |
|---|---|---|---|---|
| Which GraphDB best fits license/locality/performance/scale? | future GraphDB evaluation | before production graph substrate | yes for graph choice | LadybugDB/FalkorDB/HelixDB/other remain candidates. |
| Should durable state use SQLite, filesystem manifests, or hybrid? | future pipeline milestone | before prototype | yes | Must support resume/retry/stale detection. |
| Should workers be generic or per-sidecar? | future pipeline milestone | before sidecar worker implementation | yes | Sidecars have different latency/backend/cache needs. |
| What is the first non-paper domain? | future planning | after paper-domain stabilizes | no | Scientific articles remain first domain. |
| When may LLM/agent helpers enter? | future agent-boundary milestone | after deterministic contracts/tools | no for M034 | Agents cannot orchestrate now. |
| What review model is sufficient for readiness? | future review-boundary milestone | before import eligibility | yes | Could be deterministic, human, LLM-assisted, or hybrid. |

## Non-Authorization

Open questions do not authorize graph writes, final GraphDB selection, parser-as-truth, or agentic orchestration.
