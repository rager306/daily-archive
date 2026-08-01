# kg-pipeline

Universal pipeline DSL and execution engine for Rust.

Declarative pipeline construction with dependency validation, retry
policies, and failure handling. Stage implementations are provided by
the consuming project — kg-pipeline is stage-agnostic.

## Status

**Phase E**: skeleton with PipelineBuilder, ExecutionEngine,
FailurePolicy, RetryPolicy. 7 tests.

## Usage

```rust
use kg_pipeline::{PipelineBuilder, ExecutionEngine};

let pipeline = PipelineBuilder::new("single-paper")
    .step("ingest", "IngestPDF", &[])
    .step("extract", "ExtractEntities", &["ingest"])
    .step("enrich", "EnrichOpenAlex", &["ingest"])
    .step("validate", "ValidateGraph", &["extract", "enrich"])
    .build()
    .unwrap();

let engine = ExecutionEngine::new();
let result = engine.execute(&pipeline, |stage| async move {
    // dispatch to real stage implementation
    Ok(())
}).await;

assert!(result.success);
```

## Design

- **Stage-agnostic**: stages are string identifiers; the consuming
  project maps them to real implementations.
- **Dependency validation**: builder rejects duplicate names, unknown
  dependencies, and forward references (cycles).
- **Failure policies**: Abort, Continue, RetryThenAbort.
- See [ADR-049](../../doc/adr/ADR-049-pipeline-dsl-execution-engine.md)
  and [ADR-050](../../doc/adr/ADR-050-universal-graph-subsystem-kg-crate-family.md).

## License

MIT
