# M034 Open Questions

Open questions are not accepted decisions. They must not be treated as authorization.

| Question | Owner | Needed By | Blocking? | Evidence Required to Close | Notes |
|---|---|---|---|---|---|
| Which GraphDB best fits license/locality/performance/scale? | future GraphDB evaluation | before production graph substrate | yes for graph choice | Comparison matrix over LadybugDB, FalkorDB, HelixDB, and any new viable local-first candidate; must include license, locality, graph/vector needs, performance, portability, operational burden, export/recovery, and safety-boundary integration. | No M034/M035 work selects the final production GraphDB. Local rehearsal adapters are dry-run only unless a future graph-promotion ADR says otherwise. |
| Should durable state use SQLite, filesystem manifests, or hybrid? | M035 durable evidence pipeline prototype | before durable prototype completion | yes for prototype | Executable prototype demonstrating persisted job state, resume/retry, leases or equivalent claim semantics, stale detection by input hash/tool version, typed failure records, and deterministic tests. | M035 currently plans SQLite first because it best fits local-first and testability; this remains prototype evidence, not a production scale decision. |
| Should workers be generic or per-sidecar? | future sidecar worker milestone | before sidecar worker implementation | yes for sidecar workers | At least one no-write sidecar worker simulation with latency/backend/cache diagnostics, plus evidence showing whether generic lifecycle hooks are enough or per-sidecar adapters are required. | Sidecars have different latency, backend, cache, and failure modes. |
| What is the first non-paper domain? | future planning | after paper-domain stabilizes | no | A planning artifact that names the candidate domain, required source records, adapter boundary, evaluation corpus, and safety differences from scientific articles. | Scientific articles remain the first domain and proving ground. No second-domain assumptions should enter M035 core contracts unless they are generic and minimal. |
| When may LLM/agent helpers enter? | M035 structured review assistance and future agent-boundary milestone | after deterministic contracts/tools | no for M034 | Structured-output helper contract, redaction rules, ToolInvocationRecord trace, schema-invalid refusal tests, and proof that helper output cannot approve review, set import eligibility, or write graph data. | LLM helpers may produce diagnostics only. Agentic orchestration remains deferred until deterministic contracts, queues, review gates, and safe traces exist. |
| Who can approve a review packet for readiness? | future review-boundary milestone | before import eligibility | yes | Explicit review state machine defining deterministic validator, human, LLM-assisted, or hybrid roles; tests must prove no review packet means no readiness handoff and no readiness handoff means no import recommendation. | Approval authority must remain separate from parser, sidecar, adapter, and LLM candidate evidence producers. |

## Non-Authorization

Open questions do not authorize graph writes, final GraphDB selection, parser-as-truth, production import, review approval by LLM helper output, or agentic orchestration.

The binding safety baseline remains `doc/contracts/m034-universal-kb/SAFETY-INVARIANTS.md` until a future explicit ADR and authorized milestone supersede it.
