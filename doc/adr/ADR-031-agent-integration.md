# ADR-031: Agent Integration Plan

**Status:** Accepted (directional)  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0 S05  
**Scope:** agents / reasoning-control / tools / safety  
**Binding Level:** directional  
**Revisable:** yes, after all prerequisites met (Phase 6)

## 0. One-line Decision

> daily-archive will implement 6 SymFSM-controlled agent roles (Coordinator, SurveyWorker, CodeWikiWorker, IdeaWorker, PrototypeWorker, Aggregator) using graph operators O1-O6 as tools, with FSM-controlled reasoning (LLM as interpreter), ActiveGraph event logging, and case-based experience store. Agents are Phase 6 — the LAST phase after pipeline + queue + graph + validation.

## 1. Context

ADR-026 establishes SymFSM-inspired control direction. ADR-030 defines FalkorDB graph operators O1-O6. M048 analyzed ActiveGraph patterns. Agents must not bypass existing fail-closed safety boundaries.

## 2. Decision

### 2.1 Six Agent Roles with FSM

Each role has 5-7 FSM states with guard conditions. LLM fills structured templates, never generates freely.

### 2.2 Graph Operators as Tools

Each operator (O1-O6) is an MCP-style tool with typed input/output. Agents call tools; tools query FalkorDB read-only.

### 2.3 Safety Contract

All agents: `can_write_graph=False`, `output_type="candidate_packet"`, `requires_review=True`.

### 2.4 Experience Store

Case-based process memory (NOT model training). Stores successful FSM trajectories for reuse.

### 2.5 ActiveGraph Patterns

Event-sourced agent action log, replay, fork/diff for scenario testing.

## 3. Prerequisites (Phase 6)

Pipeline end-to-end → Queue activated → FalkorDB live → Operators O1-O6 → Safety gates → Staged validation.

## 4. LLM Reading Notes

- **Directional**: agent plan is design direction, not immediate implementation.
- **Phase 6**: agents are LAST, after everything else works.
- **Safety**: no agent writes to graph. All output is CandidatePacket.
- **SymFSM**: FSM controls reasoning, LLM fills templates.
- **Experience store**: case-based memory, not fine-tuning.
