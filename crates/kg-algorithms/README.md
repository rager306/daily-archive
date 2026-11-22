# kg-algorithms

Universal graph algorithms for temporal knowledge graphs.

Provides temporal edge resolution (the 4-rule algorithm from ADR-047),
causal chain walking, and traversal helpers. Operates on kg-ontology
types — zero project-specific dependencies.

## Status

**Current**: temporal edge resolution (`resolve_temporal_edges`, the 4-rule
ADR-047 algorithm), PageRank/personalized-PageRank (`ppr`), sparse CSR graph
kernel (`csr`) and view algorithms (degree/neighborhood helpers,
`view_algorithms`), plus the `port` trait surface. Test coverage: 39
tests over all 4
temporal rules + edge cases (transition periods, competing facts, empty input)
and the algorithm modules.

## Usage

```toml
[dependencies]
kg-algorithms = { path = "crates/kg-algorithms" }
```

```rust
use kg_algorithms::{resolve_temporal_edges, ResolutionRule};
use kg_ontology::temporal::TemporalEdge;

let new_edge = TemporalEdge::new_now();
let existing = vec![TemporalEdge::new_now()];
let outcomes = resolve_temporal_edges(&new_edge, &existing);
for o in &outcomes {
    if o.should_invalidate {
        // mark old edge as invalid_at = new_edge.valid_at
    }
}
```

## The 4-Rule Temporal Resolution

| Rule | Condition | Action |
|------|-----------|--------|
| 1. SkipOldAlreadyInvalid | old.invalid_at ≤ new.valid_at | Skip |
| 2. SkipNewWasInvalid | new.invalid_at ≤ old.valid_at | Skip |
| 3. Supersede | old.valid_at < new.valid_at | Invalidate old |
| 4. RetainBoth | Overlap, neither strictly earlier | Keep both |

See [ADR-047](../../doc/adr/ADR-047-conflict-detection-resolution.md)
and [GRAPH-CORE-TEMPORAL-DESIGN.md](../../doc/GRAPH-CORE-TEMPORAL-DESIGN.md).

## License

MIT
