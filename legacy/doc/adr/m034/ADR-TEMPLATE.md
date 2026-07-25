# Mermaid-assisted Enhanced ADR Template

Use this template for M034 ADRs. It is designed for both human readers and future LLM agents.

Core rule: **prose and tables are authoritative**. Mermaid diagrams are optional aids for structure, flows, statuses, option comparison, and contract relationships. Diagrams must clarify the ADR, not replace explicit text.

## Readability Rules

- Use at most ~3–5 Mermaid diagrams in a single ADR.
- Prefer tables for precise requirement/decision impact and option comparison.
- Keep Mermaid diagrams small; if a relationship map has more than ~8 links, move the full graph to the R/D audit artifact and keep only a small summary diagram here.
- Do not use diagrams to hide ambiguity. State the decision, non-decision, and safety boundary in prose.
- Keep class diagrams conceptual unless the ADR is specifically about contracts.
- Every ADR must include `LLM Reading Notes` so future agents can identify binding decisions and non-authorizations quickly.

---

````markdown
# ADR-XXX: <Title>

**Status:** Proposed | Accepted | Deferred | Rejected | Superseded  
**Date:** YYYY-MM-DD  
**Deciders:** human | collaborative | agent  
**Milestone:** M034-kuei9y  
**Scope:** universal-kb | graphdb | evidence-pipeline | sidecar | agent-boundary | safety | contracts  
**Binding Level:** binding | directional | exploratory  
**Revisable:** yes/no, with condition

## 0. One-line Decision

> We will <decision>.  
> We will not <explicit non-decision>.

## 1. Context

Explain why this decision exists.

Include:
- project north star;
- relevant M033 findings;
- existing Rxxx / Dxxx constraints;
- current uncertainty;
- risk if undecided.

### Context Map

```mermaid
flowchart TD
    A[Project North Star<br/>Local-first universal KB] --> B[Primary first domain<br/>Scientific articles]
    B --> C[Evidence chain]
    C --> D[Candidate artifacts]
    D --> E[Review packets]
    E --> F[Graph-readiness handoff]
    F --> G[Explicit promotion only]

    C -.supports.-> H[Sidecar pipeline]
    H --> I[GROBID]
    H --> J[OpenDataLoader]
    H --> K[Adaptix]

    G -.deferred.-> L[GraphDB choice<br/>LadybugDB / FalkorDB / HelixDB / other]
```

## 2. Decision

State the decision precisely.

Use explicit language:

```text
We will...
We will not...
This decision authorizes...
This decision does not authorize...
```

### Decision Boundary

```mermaid
flowchart LR
    IN[In scope] --> D[This ADR decision]
    D --> OUT[Out of scope]

    IN --> I1[<in-scope item 1>]
    IN --> I2[<in-scope item 2>]

    OUT --> O1[<out-of-scope item 1>]
    OUT --> O2[<out-of-scope item 2>]
```

## 3. Applies To

This decision applies to:

- generic knowledge-base architecture;
- scientific-paper first-domain implementation;
- sidecar pipeline;
- graph substrate decision;
- review/readiness gates;
- future agent workers.

### Applicability Diagram

```mermaid
flowchart TB
    ADR[ADR-XXX] --> G[Generic KB primitives]
    ADR --> P[Scientific paper adapter]
    ADR --> S[Sidecar orchestration]
    ADR --> R[Review/readiness boundary]
    ADR --> Q[Future roadmap gates]
```

## 4. Requirements and Decisions Impacted

### Requirements

| Requirement | Impact | Notes |
|---|---|---|
| R0XX | supports / constrains / supersedes / needs update | ... |

### Decisions

| Decision | Impact | Notes |
|---|---|---|
| D0XX | consistent / superseded / narrowed / needs follow-up | ... |

### R/D Relationship Map

```mermaid
flowchart TD
    ADR[ADR-XXX] --> R1[R0XX]
    ADR --> R2[R0YY]
    ADR --> D1[D0XX]
    ADR --> D2[D0YY]

    R1 -.supports.-> ADR
    D1 -.constrains.-> ADR
```

Keep this diagram small. If more than ~8 links, use tables only and move the full graph to the R/D audit artifact.

## 5. Options Considered

### Option A — <Name>

| Dimension | Assessment |
|---|---|
| Local-first fit | High / Medium / Low |
| Safety fit | High / Medium / Low |
| Complexity | High / Medium / Low |
| Reversibility | High / Medium / Low |
| GraphDB portability | High / Medium / Low |
| Agent/tooling dependency | High / Medium / Low |
| Human review compatibility | High / Medium / Low |

**Pros**
- ...

**Cons**
- ...

### Option B — <Name>

Same structure.

### Option Comparison Snapshot

```mermaid
quadrantChart
    title Option Comparison
    x-axis Low reversibility --> High reversibility
    y-axis Low safety fit --> High safety fit
    quadrant-1 Preferred
    quadrant-2 Safe but rigid
    quadrant-3 Avoid
    quadrant-4 Flexible but risky
    "Option A": [0.75, 0.85]
    "Option B": [0.40, 0.70]
    "Option C": [0.65, 0.35]
```

Use this only when it clarifies. If the chart feels forced, skip it.

## 6. Trade-off Analysis

Explain why the chosen option wins now.

Separate:

- short-term implementation value;
- long-term architecture value;
- safety impact;
- reversibility;
- cost/complexity;
- what remains uncertain.

### Trade-off Summary

| Trade-off | Chosen side | Why |
|---|---|---|
| Generic KB vs paper-only | Generic KB with paper first | Avoids overfitting while preserving current domain |
| Deterministic orchestration vs agents now | Deterministic first | Reliability risk is higher than autonomy need |
| GraphDB now vs deferred | Deferred | License/perf/locality/scaling unclear |

## 7. Consequences

### Positive

- ...

### Negative

- ...

### New obligations

- ...

### What becomes harder

- ...

### Consequence Flow

```mermaid
flowchart TD
    D[Decision] --> P[Positive consequence]
    D --> N[Negative consequence]
    D --> O[New obligation]
    O --> F[Future ADR / milestone]
```

## 8. Safety and Non-Authorization

This ADR does **not** authorize:

- production graph import;
- final GraphDB selection, unless this ADR explicitly decides that;
- LadybugDB/FalkorDB/HelixDB writes;
- parser output as graph-ready truth;
- agentic orchestration;
- bypassing validators or review packets.

Required safety defaults:

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```

### Safety Gate

```mermaid
flowchart LR
    A[Parser / Sidecar / Adapter Output] --> B[Candidate Evidence]
    B --> C[Contract Validation]
    C --> D[Review Packet]
    D --> E[Graph-readiness Review]
    E --> F{Explicit Import Authorization?}
    F -- no --> G[No-write boundary]
    F -- yes --> H[Future graph promotion milestone]
```

## 9. Contract Impact

Affected contracts:

- `KnowledgeSourceRecord`
- `DomainAdapterRecord`
- `EvidenceArtifactRecord`
- `ProcessingJob`
- `CandidatePacket`
- `ReviewPacket`
- `GraphReadinessHandoff`
- `KnowledgeSubstratePort`
- `SafetyFlags`

Required contract changes or drafts:

- ...

### Contract Relationship Map

```mermaid
classDiagram
    class KnowledgeSourceRecord {
      +source_id
      +source_type
      +source_hash
      +locality
    }

    class EvidenceArtifactRecord {
      +artifact_id
      +artifact_type
      +input_hash
      +output_path
      +producer
    }

    class ProcessingJob {
      +job_id
      +stage
      +status
      +attempt_count
      +retry_after
    }

    class CandidatePacket {
      +candidate_id
      +evidence_refs
      +review_state
    }

    class ReviewPacket {
      +packet_id
      +candidate_refs
      +diagnostics
      +review_required
    }

    class SafetyFlags {
      +graph_import_allowed
      +graphdb_written
      +import_eligible
    }

    KnowledgeSourceRecord --> EvidenceArtifactRecord
    EvidenceArtifactRecord --> CandidatePacket
    ProcessingJob --> EvidenceArtifactRecord
    CandidatePacket --> ReviewPacket
    ReviewPacket --> SafetyFlags
```

Keep classes conceptual. Do not over-specify implementation fields in ADR unless the ADR is specifically about contracts.

## 10. Validation / Evidence Required

What evidence is needed to accept or revisit this ADR?

Examples:

- document consistency audit;
- R/D consistency audit;
- future GraphDB comparison matrix;
- local prototype;
- failure-mode test;
- status transition verification;
- no-write rehearsal;
- reader test.

### Validation Path

```mermaid
flowchart TD
    A[ADR accepted/deferred] --> B[Required artifacts]
    B --> C[Verification checklist]
    C --> D{Pass?}
    D -- yes --> E[Ready for next milestone]
    D -- no --> F[Correction or user discussion]
```

## 11. Open Questions

| Question | Owner | Needed by | Blocking? |
|---|---|---|---|
| ... | ... | ... | yes/no |

## 12. Follow-up Actions

- [ ] ...
- [ ] ...

## 13. Supersedes / Superseded By

### Supersedes

- D0XX, if applicable

### Superseded By

- empty until future ADR

## 14. LLM Reading Notes

This section is intentionally explicit for future agents.

- Binding decision:
  - ...
- Do not infer:
  - ...
- Safe next action:
  - ...
- Blocked until:
  - ...
````

---

## Special Mermaid Blocks by ADR Type

Use these only when they improve readability.

### GraphDB ADR

```mermaid
flowchart TD
    A[KnowledgeSubstratePort] --> B[LadybugDB candidate]
    A --> C[FalkorDB candidate]
    A --> D[HelixDB candidate]
    A --> E[Other candidates]

    B --> B1[License]
    B --> B2[Locality]
    B --> B3[Performance]
    B --> B4[Graph-vector support]

    C --> C1[License]
    C --> C2[Locality]
    C --> C3[Performance]
    C --> C4[Graph-vector support]
```

### Queue / Status ADR

```mermaid
stateDiagram-v2
    [*] --> pending
    pending --> ready
    ready --> running
    running --> succeeded
    running --> failed_retryable
    failed_retryable --> ready
    running --> failed_terminal
    running --> blocked
    succeeded --> stale
    stale --> ready
    blocked --> ready
```

### Evidence-chain ADR

```mermaid
flowchart LR
    A[Knowledge Source] --> B[Source Record]
    B --> C[Evidence Artifact]
    C --> D[Candidate Packet]
    D --> E[Review Packet]
    E --> F[Readiness Handoff]
    F --> G{Promotion Authorized?}
    G -- no --> H[No-write Boundary]
    G -- yes --> I[Future GraphDB Write Milestone]
```

### Universal KB vs Paper Domain ADR

```mermaid
flowchart TD
    U[Universal KB Core] --> S[KnowledgeSourceRecord]
    U --> A[EvidenceArtifactRecord]
    U --> J[ProcessingJob]
    U --> C[CandidatePacket]
    U --> R[ReviewPacket]

    U --> P[Scientific Paper Domain Adapter]
    P --> P1[GROBID sidecar]
    P --> P2[OpenDataLoader sidecar]
    P --> P3[Adaptix mapping]
    P --> P4[Paper review packet]

    U --> F[Future domain adapters]
    F --> F1[Web articles]
    F --> F2[Books/reports]
    F --> F3[Datasets/docs]
```

---

## Final Reminder

Use this template to preserve structure, not to create bureaucracy.

- Text is always primary.
- Tables carry exact R/D impact and option comparisons.
- Mermaid clarifies structure, flow, states, and contracts.
- `LLM Reading Notes` make the ADR safe for future agents to consume.
