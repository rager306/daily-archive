# ADR-027: Resource-Aware Queue Scheduler

**Status:** Accepted  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0  
**Scope:** queue / scheduler / resource-management / llm  
**Binding Level:** binding  
**Revisable:** yes, with implementation evidence

## 0. One-line Decision

> The pipeline queue scheduler must coordinate job dispatch across three resource dimensions: **LLM API rate limits** (per-provider token/time quotas), **CPU/local compute** (parser/heavy processing), and **I/O** (network fetch, disk write). Jobs are routed to the least-constrained resource lane and backoff when all lanes are saturated.

## 1. Context

ADR-017 deferred the queue until the pipeline is end-to-end complete. ADR-025 established multi-provider LLM with per-provider rate limit checking. However, rate limits are only one dimension of resource contention:

- **MiniMax** has token_plan limits (checkable via `token_plan/remains` endpoint)
- **GLM** has 5-hour rolling subscription limits
- **Marker/GROBID** parsers are CPU-heavy (Marker can saturate a single core for minutes)
- **PDF downloads** are I/O-bound with arXiv rate limiting (429s)
- **Embedding service** (fd/TEI on localhost:8000) has its own capacity
- **FalkorDB writes** need to be serialized during migration

A naive queue that only checks LLM limits will either:
1. Over-subscribe CPU (multiple Marker instances → system freeze)
2. Under-utilize LLM (wait for CPU job while LLM is idle)
3. Hit I/O bottlenecks (parallel arXiv downloads → 429 bans)

## 2. Decision

### 2.1 Three-Lane Resource Model

```text
┌─────────────────────────────────────────────┐
│              Queue Scheduler                 │
│                                              │
│  ┌─────────────┐ ┌─────────┐ ┌────────────┐│
│  │ LLM Lane    │ │CPU Lane │ │ I/O Lane   ││
│  │             │ │         │ │            ││
│  │ MiniMax:    │ │ Marker  │ │ PDF fetch  ││
│  │  token_plan │ │ GROBID  │ │ arXiv API  ││
│  │  check      │ │ PyMuPDF │ │ disk write ││
│  │             │ │ TEI emb │ │ FalkorDB   ││
│  │ GLM:        │ │         │ │            ││
│  │  5h rolling │ │ n_jobs  │ │ n_jobs     ││
│  │  counter    │ │ ≤ ncpu-1│ │ ≤ 4       ││
│  └─────────────┘ └─────────┘ └────────────┘│
│                                              │
│  Each lane: independent concurrency limit    │
│  Job tagged with required lane(s)            │
│  Multi-lane jobs: reserve ALL required lanes │
└─────────────────────────────────────────────┘
```

### 2.2 Job Resource Tags

Every `ProcessingJob` in the queue carries a `resource_profile`:

```python
@dataclass(frozen=True)
class ResourceProfile:
    """Declares what resources a job needs."""
    # LLM requirements
    llm_required: bool = False
    llm_provider: str | None = None  # "minimax" | "glm" | None=any
    estimated_tokens: int = 0

    # CPU requirements
    cpu_required: bool = False
    cpu_intensity: str = "light"  # "light" | "medium" | "heavy"

    # I/O requirements
    io_required: bool = False
    io_type: str | None = None  # "network" | "disk" | "graph_write"
```

### 2.3 Per-Lane Admission Control

| Lane | Limit source | Check mechanism | When saturated |
|---|---|---|---|
| **LLM (MiniMax)** | token_plan/remains endpoint | `minimax_usage.py` → HTTP GET before dispatch | Fallback to GLM lane |
| **LLM (GLM)** | 5-hour rolling window | In-memory counter + timestamp; reset every 5h | Queue job for later |
| **CPU** | Physical CPU count | `os.cpu_count() - 1` concurrent heavy jobs | Backoff + retry |
| **I/O (network)** | arXiv rate limit (429 aware) | Sliding window: max 3 concurrent fetches | Exponential backoff on 429 |
| **I/O (disk)** | Disk write throughput | Sequential for large writes; parallel for small | Auto-throttle |
| **I/O (graph_write)** | FalkorDB write lock | Single-writer during migration phase | Queue after current write |

### 2.4 Scheduler Algorithm

```python
def schedule_next(jobs: list[ProcessingJob], resources: ResourceState) -> ProcessingJob | None:
    """Pick the next job that can run given current resource availability."""
    for job in jobs.sorted_by(priority, created_at):
        profile = job.resource_profile
        if profile.llm_required and not can_make_llm_request(profile.llm_provider, profile.estimated_tokens):
            continue  # LLM lane full for this provider
        if profile.cpu_required and cpu_slots_available() == 0:
            continue  # CPU lane full
        if profile.io_required and not io_slots_available(profile.io_type):
            continue  # I/O lane full
        return job  # All required lanes have capacity
    return None  # All jobs blocked; wait for resource release
```

**Priority order**:
1. Interactive/review jobs (user waiting)
2. Extraction jobs with LLM slots available (don't waste API quota)
3. CPU-heavy parsing jobs (long-running, start early)
4. I/O fetch jobs (network-bound, can parallelize)
5. Maintenance/cleanup jobs (background)

### 2.5 Backoff and Recovery

| Condition | Action |
|---|---|
| MiniMax 429 (rate limited) | Switch to GLM; log to queue with `rate_limited` status |
| GLM 429 (rate limited) | Queue all LLM jobs; check MiniMax again in 60s |
| All LLM providers exhausted | Pause LLM lane; continue CPU and I/O lanes |
| CPU saturated (load > ncpu) | Reduce CPU concurrency by 1; backoff 30s |
| arXiv 429 | Exponential backoff: 60s, 120s, 300s, 600s |
| FalkorDB write conflict | Retry with jitter; max 3 retries |
| Embedding service down | Circuit breaker (already in `embedder.py`); graceful degradation |

### 2.6 Observability

The scheduler exposes:

```python
@dataclass(frozen=True)
class SchedulerStatus:
    llm_minimax_available: bool
    llm_glm_available: bool
    llm_minimax_remaining_tokens: int | None
    cpu_current_jobs: int
    cpu_max_jobs: int
    io_network_current: int
    io_network_max: int
    queue_depth: int
    queue_depth_by_lane: dict[str, int]
    last_rate_limit_at: datetime | None
    last_rate_limit_provider: str | None
```

This feeds into the trajectory checker (M045) and CLI status output.

### 2.7 Relationship to Existing Components

| Component | Role | Interaction with scheduler |
|---|---|---|
| `workflows/universal_kb/queue.py` | Job storage (SQLite) | Scheduler reads pending jobs from queue |
| `llm/minimax_usage.py` | MiniMax limit checking | Scheduler calls before LLM dispatch |
| `llm/provider_config.py` | Provider configuration | Scheduler uses for provider routing |
| `retrieval/embedder.py` | fd embedding service | Scheduler checks circuit breaker state |
| `graph/ladybug_client.py` → FalkorDB | Graph writes | Scheduler serializes graph_write lane |

## 3. Applies To

- All pipeline stages: fetch, parse, chunk, extract, review, import
- Agent dispatch (future, when agents are activated)
- CLI status reporting
- Trajectory checker (M045) resource dimension

## 4. Requirements and Decisions Impacted

### Requirements

| Requirement | Impact | Notes |
|---|---|---|
| R069 | extends | Per-provider rate limits now part of multi-lane scheduler |
| R024 | supports | Staged validation needs reliable job dispatch without resource exhaustion |

### Decisions

| Decision | Impact | Notes |
|---|---|---|
| ADR-017 | activates | Queue was deferred; scheduler design prepares for activation |
| ADR-025 | extends | Multi-provider rate limits now coordinated with CPU and I/O |
| ADR-023 | consistent | Scheduler is Layer 6 (Pipeline and Queue) of 7-layer architecture |

## 5. Implementation Phasing

```
Phase 2 (extraction prototype):
  - Simple LLM lane: check MiniMax token_plan before each call
  - No CPU/I/O coordination yet

Phase 3 (FalkorDB migration):
  - Add graph_write lane (single-writer)
  - Add CPU lane for heavy graph operations

Phase 4 (staged validation):
  - Full 3-lane scheduler
  - SQLite-backed queue with resource_profile
  - Backoff and recovery

Phase 5+ (universal ingestion + agents):
  - Multi-source I/O coordination
  - Agent dispatch with FSM state management
```

## 6. Safety

- Scheduler NEVER bypasses safety gates (SafetyFlags remain false)
- Scheduler NEVER writes to graph directly
- Scheduler tracks resource state, NOT knowledge content
- Rate limit failures are typed FailureRecords, not silent drops

## 7. LLM Reading Notes

- **Binding**: Three-lane resource model (LLM, CPU, I/O) is mandatory for the queue.
- **Phased**: Simple LLM lane first (Phase 2); full 3-lane in Phase 4.
- **ADR-017**: Queue activation still deferred until pipeline end-to-end, but scheduler design is ready.
- **Observability**: SchedulerStatus feeds trajectory checker and CLI.
- **Not authorized**: graph writes, fact promotion, agent dispatch (deferred).
