# ADR-052: Structured Logging — Tracing with Dual-Layer JSON File + Stderr

**Status:** Accepted
**Date:** 2026-08-05
**Deciders:** collaborative
**Related:**
- ADR-034 (hexagonal/onion overlay — logging spans are cross-cutting)
- ADR-041 (Samyama embedded — pipeline needs post-mortem debugging)
- ADR-051 (testing strategy — logs are verification evidence)
- M359 (implementation milestone)

## Context

daily-archive had **stderr-only** logging via `tracing_subscriber::fmt().init()`.
No file output, no JSON, no span hierarchy, no rolling rotation. When an
ingest or extraction failed, the operator had to scroll terminal history
or re-run the command. Future agents could not parse logs programmatically.

Key gaps identified:

1. **No file persistence** — logs evaporate when terminal closes.
2. **No structured fields** — messages were free-text; impossible to
   filter by `paper_id`, `node_count`, `duration_ms`.
3. **No span hierarchy** — async tasks interleave; no way to trace
   `ingest_pdf → parse → extract → graph_write` as a tree.
4. **No rotation** — a single file would grow unbounded.
5. **No agent-readable format** — future agents need machine-parseable
   JSON logs for post-mortem analysis without re-running the pipeline.

## Research (2025-2026 Best Practices)

Sources: tracing crate docs, tracing-appender WorkerGuard docs,
oneuptime.com structured logging guides (2026-01, 2026-02),
shuttle.dev pipeline monitoring (2025-09), opentelemetry.io Rust
getting started (2026-05), groundcover distributed tracing, comet.com
agent observability.

### Key Findings

1. **Dual-layer subscriber** is the industry standard: human-readable
   stderr + machine-parseable JSON file. Confirmed across all sources.

2. **`with_ansi(false)`** on file layer is critical — ANSI escape codes
   corrupt JSON structure and break `jq` parsing. Every source warns
   about this.

3. **`flatten_event(true)`** puts event fields at the JSON top level
   (not nested under `"fields": {}`), enabling direct jq queries like
   `jq 'select(.paper_id == "...")'`.

4. **`with_span_events(FmtSpan::CLOSE)`** logs span close events with
   duration — automatically captures timing for every `#[instrument]`-ed
   function without manual `Instant::now()` boilerplate.

5. **`WorkerGuard` lifetime** is the #1 tracing footgun: the guard
   must live in `main()` scope, not inside `init_logging()`. If dropped
   early, the non-blocking writer silently stops flushing. Documented in
   `tracing-appender` official docs.

6. **`#[instrument(skip(large_field))]`** prevents serializing large
   arguments (embeddings, PDF bytes, graph snapshots) into every span.

7. **Per-layer filtering** (stderr=INFO, file=DEBUG) gives operators
   a clean console while capturing full trace for debugging.

8. **`try_init()`** not `init()` — don't panic if subscriber already
   set (happens in tests and nested runtimes).

9. **Async context preservation**: `tokio::spawn` tasks must use
   `.instrument(span.clone())` to carry span context across thread
   boundaries. The `#[instrument]` macro handles this automatically
   for `async fn`.

10. **Structured span fields** (`paper_id`, `node_count`, `duration_ms`)
    propagate to all child events automatically — no manual threading.

## Decision

Adopt **tracing + tracing-subscriber + tracing-appender** with a
dual-layer architecture for all `da` CLI commands.

### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    init_logging()                             │
│                                                              │
│  ┌──────────────────┐  ┌─────────────────────────────────┐  │
│  │  stderr layer    │  │  file layer                     │  │
│  │  compact, ANSI   │  │  JSON, no ANSI                   │  │
│  │  shared filter   │  │  shared filter                   │  │
│  │  human-readable  │  │  flatten_event(true)             │  │
│  │                  │  │  with_span_events(CLOSE)         │  │
│  │  → terminal      │  │  → logs/da.log.YYYY-MM-DD        │  │
│  └──────────────────┘  └─────────────────────────────────┘  │
│         ↑                          ↑                         │
│         └────── WorkerGuard ───────┘                         │
│                (held in main())                               │
└──────────────────────────────────────────────────────────────┘
```

### Configuration (env vars)

| Var | Default | Purpose |
|-----|---------|---------|
| `LOG_DIR` | `logs/` | Directory for rolling log files |
| `LOG_LEVEL` | `info` | Shared stderr/file filter (`RUST_LOG` overrides if set) |

The file format is intentionally fixed to JSON for machine-readable evidence;
`LOG_FORMAT` is not a supported runtime option.

### Span Instrumentation Contract

Pipeline functions **must** use `#[tracing::instrument]` or structured lifecycle
events. Every active CLI example composition root calls
`init_pipeline_logging`, which loads `.env`, installs stderr + rolling JSON and
holds its `WorkerGuard` until process exit.

Lifecycle events carry identity and state, not prose alone:

- cache stages: `event`, `article_id`, `pipeline_id`, `step`, `stage`, disposition/checksum;
- graph materializers: `event`, `paper_id`, node/edge/vector write deltas and D127 state;
- failures: stable error class, retry time, payload size/hash — never raw provider text or secrets.


```rust
#[tracing::instrument(skip(self, pdf_bytes), fields(paper_id = %paper_id))]
pub async fn ingest_pdf(&self, pdf_path: &str, paper_id: &str) -> Result<...> { ... }
```

Required spans (hierarchy):

```
ingest_pdf
  ├── pdf_classify (pre-GROBID triage)
  ├── parse_pdf (GROBID TEI)
  ├── embed (FD_API bge-m3)
  ├── graph_write (Samyama create_node × N)
  └── extract (RuleBasedExtractor)
       ├── entity_extract
       ├── evidence_bundle
       └── edge_create (MENTIONS, FOUND_IN)
```

### JSON Log Schema (file layer)

```json
{
  "timestamp": "2026-08-05T17:25:00.123Z",
  "level": "INFO",
  "target": "da_application::ingest",
  "span": {
    "name": "ingest_pdf",
    "paper_id": "2507.19457"
  },
  "paper_id": "2507.19457",
  "pdf_type": "Text",
  "page_count": 96,
  "duration_ms": 4500,
  "message": "Ingest complete"
}
```

### Querying Logs (for future agents and operators)

```sh
# All events for a specific paper
jq 'select(.paper_id == "2507.19457")' logs/da.log.2026-08-05

# All errors
jq 'select(.level == "ERROR")' logs/da.log.2026-08-05

# All events within ingest_pdf span
jq 'select(.span.name == "ingest_pdf")' logs/da.log.2026-08-05

# Slow operations (>1s)
jq 'select(.duration_ms > 1000)' logs/da.log.2026-08-05

# Span close events with timing
jq 'select(.event == "span_close")' logs/da.log.2026-08-05
```

## Consequences

### Positive

- **Post-mortem debugging**: operators and agents can replay any pipeline
  run from JSON logs without re-running the command.
- **Span hierarchy**: async task interleaving is disambiguated by span
  context propagation.
- **Performance**: `#[instrument]` with `skip(large_field)` adds negligible
  overhead; non-blocking writer runs on dedicated thread.
- **Rotation**: daily files auto-rotate; old files can be archived or
  deleted without stopping the process.
- **Agent-readable**: JSON logs are parseable by jq, Python, or future
  LLM agents for automated diagnostics.
- **Single filter contract**: `RUST_LOG` or `LOG_LEVEL` applies predictably to
  both surfaces; INFO semantic lifecycle events remain available at defaults.

### Negative

- **Disk usage**: DEBUG+ JSON logs can grow. Mitigated by daily rotation;
  operator can set `LOG_LEVEL=info` to reduce file verbosity.
- **WorkerGuard footgun**: if guard drops early, file logs silently stop.
  Documented in init function and this ADR; code review must check.
- **Non-deterministic flush timing**: non-blocking writer may not flush
  the last few events on abrupt termination (panic, SIGKILL). Graceful
  shutdown via WorkerGuard drop handles normal exit.

### Neutral

- **tracing-appender** adds one dependency (`tracing-appender = "0.2"`).
- **JSON feature** on tracing-subscriber already available in workspace.

## Implementation

- `crates/da-cli/src/logging.rs` — `init_logging() -> Option<WorkerGuard>`
- `crates/da-cli/src/main.rs` — holds `_log_guard` in `main()` scope
- `.env.example` — `LOG_DIR`, `LOG_LEVEL`, `LOG_FORMAT` documented
- `#[tracing::instrument]` on `ingest_pdf`, `extract_paper`, `batch_ingest_pdfs`
- `#[instrument(skip(large_field))]` on functions with embeddings/PDF bytes

## Alternatives Considered

1. **stderr-only with `RUST_LOG`** — rejected: no file persistence,
   no JSON, no rotation, no post-mortem capability.

2. **`slog` structured logger** — rejected: `tracing` is the Rust
   ecosystem standard with span support; `slog` lacks async span
   hierarchy and OpenTelemetry bridge.

3. **OpenTelemetry-only** — rejected as premature: requires collector
   infrastructure (Jaeger/OTLP endpoint). The dual-layer file approach
   gives 90% of the value with zero infrastructure. Can add OTel layer
   later without changing instrumentation code (tracing is compatible).

4. **`env_logger`** — rejected: no span support, no structured fields,
   no async context propagation. `tracing` subsumes `env_logger` and
   bridges `log` crate via feature flag.
