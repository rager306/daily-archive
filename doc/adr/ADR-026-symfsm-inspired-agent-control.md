# ADR-026: Agent Integration via SymFSM-Inspired Control Structures

**Status:** Accepted (design direction)  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0  
**Scope:** agents / reasoning-control / safety  
**Binding Level:** directional  
**Revisable:** yes, after pipeline + graph are operational

## 0. One-line Decision

> Future agents will use FSM-controlled reasoning inspired by SymFSM: the LLM is an interpreter within a pre-built reasoning model, NOT the "brain". Agents operate through typed graph operators with safety gates, not free-form generation.

## 1. Context

SymFSM proposes shifting AI system control from LLM to formal reasoning structures. The LLM fills templates within a state machine, while the FSM controls transitions, verifies structure, and repairs gaps.

This is compatible with our existing architecture:
- **Cognitive map** = subgraph of FalkorDB (already have graph + typed relations planned)
- **Repair engine** = graph operators O5 (gap detection) + O6 (novelty grounding)
- **Structural verifier** = typed relation path checking
- **Safety gates** = existing fail-closed boundaries (SafetyFlags)

## 2. Decision

### 2.1 Agent Control Flow

```text
User Query
   → Task Interpreter (classify: explain/compare/design/analyze/plan)
   → Cognitive Map Builder (query FalkorDB for relevant subgraph)
   → Structural Verifier (is answer reachable? are there gaps?)
   → Repair Engine (if gaps: clarify, decompose, or reframe via graph operators)
   → LLM Generator (fill structured template from verified cognitive map)
   → Output Verifier (check against map + safety flags)
   → Experience Store (save reasoning pattern for future use)
```

### 2.2 Agent Roles (from Agents-K1 + ActiveGraph)

| Role | FSM states | Graph operators | LLM role |
|---|---|---|---|
| Coordinator | INTERPRET → PLAN → DISPATCH → AGGREGATE | O1 (seed) | Classify task, select workers |
| SurveyWorker | QUERY → CLUSTER → SYNTHESIZE → VERIFY | O2 (lineage), O3 (baseline) | Write topic summaries |
| CodeWikiWorker | FETCH_CODE → ANALYZE → DOCUMENT | O4 (multimodal) | Document code communities |
| IdeaWorker | GROUND → GENERATE → CRITIQUE → REFINE | O5 (gap), O6 (novelty) | Generate + evaluate ideas |
| PrototypeWorker | SPECIFY → SCAFFOLD → VALIDATE | O3 (baseline) | Method spec → code scaffold |
| Aggregator | COLLECT → MERGE → MANIFEST | All | Merge artifacts, write manifest |

### 2.3 Safety Integration

Every agent action passes through:
1. `SafetyFlags` check (all false by default)
2. `CandidatePacket` → `ReviewPacket` pipeline
3. `Import Gate` (fail-closed) for any graph modification
4. Output Verifier checks claims against FalkorDB evidence paths

### 2.4 Experience Store

Not model fine-tuning. Case-based process memory:
- Successful reasoning patterns (FSM trajectories)
- Repair strategies that worked
- Query → subgraph mappings
- Failed approaches (negative examples)

Stored as typed records in the queue/graph, not in model weights.

### 2.5 What We Do NOT Adopt from SymFSM

- 30D State Inspector (over-engineered)
- Proprietary cognitive kernel runtime
- Full formal logic layer (use typed relations + graph algorithms instead)

## 3. Prerequisites

Agent integration is **DEFERRED** until:
1. ✅ Pipeline (parser → structure → extraction → graph) is operational
2. ✅ Queue is activated (ADR-017)
3. ✅ FalkorDB schema with typed relations is live
4. ✅ Graph operators O1-O6 are implemented
5. ✅ Safety gates handle typed entities
6. ✅ Staged validation (R024: 10→20→week) proves graph quality

## 4. LLM Reading Notes

- **Directional**: SymFSM-inspired agents are the design direction, not immediate implementation.
- **LLM role**: interpreter within FSM, NOT free generator.
- **Agents are deferred** until pipeline + queue + graph are operational.
- **Safety**: every agent action through fail-closed gates.
- **Experience store**: case-based memory, not model training.
