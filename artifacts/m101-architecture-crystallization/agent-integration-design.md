# Agent Integration Design (M101 S05)

## Overview

Designs SymFSM-inspired multi-agent system (ADR-026) integrating with FalkorDB graph operators (ADR-030), ActiveGraph patterns (M048), and existing fail-closed safety boundaries.

## Core Principle

> **LLM is an interpreter within a pre-built reasoning model, NOT the "brain".** The FSM controls transitions; the LLM fills structured templates.

## Agent Roles (6)

### 1. Coordinator

**Purpose**: Analyze query + graph state → produce execution plan.

```text
FSM States:
  INIT → ANALYZE_QUERY → SELECT_WORKERS → DISPATCH → MONITOR → AGGREGATE → DONE

Guard conditions:
  ANALYZE_QUERY → SELECT_WORKERS: query_type classified
  SELECT_WORKERS → DISPATCH: at least one worker selected
  DISPATCH → MONITOR: all dispatched jobs have resource_profile assigned
  MONITOR → AGGREGATE: all workers completed OR timeout
  AGGREGATE → DONE: manifest written with all results
```

**Graph operators used**: O1 (Seed Resolution) for query entity resolution.

**LLM role**: Classify query type (explain/compare/design/analyze/plan), select workers based on query type and graph coverage.

**Safety**: Coordinator cannot write to graph. It only reads graph state and dispatches jobs.

### 2. SurveyWorker

**Purpose**: Cluster paper nodes → write topic sections with citation lineage.

```text
FSM States:
  INIT → QUERY_GRAPH → CLUSTER_NODES → SYNTHESIZE_SECTIONS → VERIFY_CITATIONS → DONE

Guard conditions:
  QUERY_GRAPH → CLUSTER_NODES: subgraph has ≥3 nodes
  CLUSTER_NODES → SYNTHESIZE_SECTIONS: clusters have labels
  SYNTHESIZE_SECTIONS → VERIFY_CITATIONS: sections written
  VERIFY_CITATIONS → DONE: all citations traced to Evidence nodes
```

**Graph operators**: O2 (Citation Lineage), O3 (Comparative Baseline).

**LLM role**: Write coherent topic summaries from clustered subgraphs. NOT free-form — structured template with evidence IDs.

**Safety**: Output is CandidatePacket (safety_flags=false). Citations must trace to Evidence nodes.

### 3. CodeWikiWorker

**Purpose**: Read code communities → write repo documentation.

```text
FSM States:
  INIT → FETCH_CODE → ANALYZE_COMMUNITIES → DOCUMENT → CROSS_LINK → DONE

Guard conditions:
  FETCH_CODE → ANALYZE_COMMUNITIES: code components ingested
  ANALYZE_COMMUNITIES → DOCUMENT: communities identified with entry points
  DOCUMENT → CROSS_LINK: docs reference code component IDs
```

**Graph operators**: O4 (Multimodal Anchor) for code snippets, O1 for cross-referencing.

**LLM role**: Generate documentation for code communities from AST + README + docstrings.

**Safety**: Documentation is CandidatePacket. No code execution, no API calls.

### 4. IdeaWorker

**Purpose**: Generate, ground, and evaluate research ideas.

```text
FSM States:
  INIT → GROUND_CHECK → GENERATE → CRITIQUE → REFINE → NOVELTY_CHECK → DONE

Guard conditions:
  GROUND_CHECK → GENERATE: existing work mapped (no blind generation)
  GENERATE → CRITIQUE: at least 3 ideas generated
  CRITIQUE → REFINE: ideas scored with rationale
  REFINE → NOVELTY_CHECK: refined ideas have mechanism description
  NOVELTY_CHECK → DONE: each idea has novelty score + overlap report
```

**Graph operators**: O5 (Gap Detection) for opportunity areas, O6 (Novelty Grounding) for overlap check.

**LLM role**: Generate ideas from gap report. Fill structured idea template (problem, mechanism, expected_outcome, related_work_ids).

**Safety**: Ideas are CandidatePacket with `import_eligible=false`. No graph writes.

### 5. PrototypeWorker

**Purpose**: Turn idea into method specification + code scaffold.

```text
FSM States:
  INIT → SPECIFY_METHOD → GENERATE_SCAFFOLD → VALIDATE_INTERFACE → DONE

Guard conditions:
  SPECIFY_METHOD → GENERATE_SCAFFOLD: method spec has inputs/outputs/steps
  GENERATE_SCAFFOLD → VALIDATE_INTERFACE: scaffold compiles (syntax check)
  VALIDATE_INTERFACE → DONE: interface matches method spec
```

**Graph operators**: O3 (Comparative Baseline) for existing implementations.

**LLM role**: Generate method specification and code scaffold from idea template.

**Safety**: Scaffold is CandidatePacket. No execution, no dependency installation.

### 6. Aggregator

**Purpose**: Merge worker outputs → write manifest.

```text
FSM States:
  INIT → COLLECT_ARTIFACTS → MERGE → RESOLVE_CONFLICTS → WRITE_MANIFEST → DONE

Guard conditions:
  COLLECT_ARTIFACTS → MERGE: all expected artifacts received
  MERGE → RESOLVE_CONFLICTS: merge completed (even with conflicts)
  RESOLVE_CONFLICTS → WRITE_MANIFEST: conflicts logged with resolution strategy
  WRITE_MANIFEST → DONE: manifest references all evidence IDs
```

**Graph operators**: None directly. Aggregator reads worker manifests.

**LLM role**: Resolve minor conflicts (e.g., duplicate entity names). Flag major conflicts for human review.

**Safety**: Manifest has safety_flags=false. Aggregator cannot write to graph.

## Tool Definitions (MCP-style)

Each graph operator is exposed as a tool:

```python
# Tool: seed_resolution (O1)
class SeedResolutionTool:
    """Resolve mention strings to canonical entity nodes."""
    def __call__(self, mentions: list[str]) -> list[EntityResolution]:
        # FalkorDB: exact match + vector similarity > 0.85
        ...

# Tool: citation_lineage (O2)
class CitationLineageTool:
    """Reconstruct citation lineage forward/backward."""
    def __call__(self, source_id: str, direction: str = "both", depth: int = 2) -> CitationGraph:
        # FalkorDB: shortestPath via CITES edges
        ...

# Tool: comparative_baseline (O3)
class ComparativeBaselineTool:
    """Find methods evaluated on a given dataset/metric."""
    def __call__(self, dataset: str, metric: str | None = None) -> list[MethodComparison]:
        # FalkorDB: APPLIED_TO + TARGETS traversal
        ...

# Tool: multimodal_anchor (O4)
class MultimodalAnchorTool:
    """Retrieve figures, tables, equations by semantic similarity."""
    def __call__(self, query: str, content_type: str = "all", limit: int = 10) -> list[ContentAnchor]:
        # FalkorDB: vector query on Entity nodes with type filter
        ...

# Tool: gap_detection (O5)
class GapDetectionTool:
    """Find orphan methods, singleton datasets, sparse areas."""
    def __call__(self, domain: str | None = None) -> GapReport:
        # FalkorDB: pattern matching for orphans/singletons
        ...

# Tool: novelty_grounding (O6)
class NoveltyGroundingTool:
    """Check if a proposed idea overlaps with existing work."""
    def __call__(self, problem: str, techniques: list[str]) -> NoveltyReport:
        # FalkorDB: SOLVES + USES_TECHNIQUE overlap
        ...
```

## Safety Boundary Integration

### Per-agent safety contract

```python
@dataclass(frozen=True)
class AgentSafetyContract:
    """Safety constraints for agent execution."""
    agent_role: str
    can_read_graph: bool = True          # All agents can read
    can_write_graph: bool = False        # NO agent writes directly
    can_call_llm: bool = True            # All agents can use LLM (rate-limited)
    can_execute_code: bool = False       # NO code execution
    can_access_network: bool = False     # NO network access
    output_type: str = "candidate_packet"  # Always candidate, never truth
    requires_review: bool = True         # Always requires human review
```

### FSM → Safety Gate Flow

```text
Agent FSM State Transition
    ↓
Guard Condition Check
    ↓
SafetyFlags Verification (all must be false)
    ↓
Graph Read (if needed) ← read-only, no mutation
    ↓
LLM Call (if needed) ← rate-limited, provider-routed
    ↓
Output → CandidatePacket (safety_flags=false)
    ↓
CandidatePacket → Review Gate → Import Gate → FalkorDB Write
                                        ↑
                                   fail_closed
```

### ActiveGraph Pattern Integration (M048)

| ActiveGraph Pattern | Agent Integration |
|---|---|
| **Event-sourced log** | Agent actions logged as events (role, state, tool, result) |
| **Behaviors** | Each agent role = a behavior subscribed to query events |
| **Replay** | Given event log, replay agent execution deterministically |
| **Fork/diff** | Fork agent run with different query → compare results |
| **Cache replay** | Shared prefix between fork and original reused |

## Experience Store (Case-Based Process Memory)

```python
@dataclass(frozen=True)
class ReasoningExperience:
    """Saved reasoning pattern for future use."""
    experience_id: str
    query_type: str                    # explain/compare/design/analyze/plan
    fsm_trajectory: list[str]          # sequence of states visited
    tools_used: list[str]              # operators invoked
    repair_strategies: list[str]       # repairs applied (if any)
    outcome: str                       # success/partial/failed
    duration_ms: int
    llm_calls: int
    evidence_ids: list[str]            # evidence used
```

**NOT model fine-tuning.** Case-based memory stored as typed records.

**Retrieval**: when a new query arrives, Coordinator searches Experience Store for similar query_type → reuses successful trajectory pattern.

## Coordination Protocol

```text
User Query
    ↓
Coordinator: classify query → select workers → dispatch jobs
    ↓
Jobs enter queue with ResourceProfile(agent_required=True, llm_required=True)
    ↓
3-lane Scheduler dispatches when resources available (ADR-027)
    ↓
Workers execute FSM → use tools → produce CandidatePackets
    ↓
Aggregator: merge → resolve conflicts → manifest
    ↓
Output: manifest + candidate packets (for human review)
```

## Prerequisites (Must Be Met Before Agent Activation)

| Prerequisite | Milestone | Status |
|---|---|---|
| Pipeline end-to-end (parse→chunk→extract→graph) | Phase 2 | ⬜ |
| Queue activated (ADR-017) | Phase 3 | ⬜ |
| FalkorDB with typed relations live | Phase 3 | ⬜ |
| Graph operators O1-O6 implemented | Phase 3 | ⬜ |
| Safety gates handle typed entities | Phase 3 | ⬜ |
| Staged validation (R024: 10→20→week) | Phase 4 | ⬜ |

**Agents are Phase 6** — the LAST phase.

## What We Do NOT Adopt

| From SymFSM | Why not |
|---|---|
| 30D State Inspector | Over-engineered; our FSM has ~5-7 states per agent |
| Proprietary cognitive kernel runtime | We use Python + FalkorDB + FSM |
| Full formal logic layer | Typed relations + graph algorithms are sufficient |

| From ActiveGraph | Why not |
|---|---|
| Postgres event log | FalkorDB (Redis-based) suffices for our scale |
| Distributed runtime | Single-process FIFO queue is enough (ADR-017) |
| Full reactive graph projection | FalkorDB IS the graph projection |
