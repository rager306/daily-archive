# rustworkx library research

## Architecture summary

GitNexus context found `PyDiGraph` in `src/digraph.rs` with ambiguous struct/impl matches, which is expected for a Rust-backed Python extension. The public Python graph surface centers on `PyDiGraph` and exposes traversal, centrality/link-analysis, shortest path, and component algorithms via Rust implementations.

For our M060b intermediate layer, rustworkx is a plausible accelerator where low-latency traversal and path queries matter. It is less direct than NetworkX for authoring but can be populated from our manifest and used as a read-only algorithm backend. Graph writes are not authorized; production import is not authorized; fact promotion is not authorized; external network default is disabled; LLM calls default is disabled.

## Algorithm support table

| Algorithm | Support | Evidence |
|---|---:|---|
| BFS | Yes | GitNexus query surfaced traversal/graph process symbols; local vendored hits include `src/iterators.rs` and `rustworkx/__init__.py`. |
| PageRank | Yes | Local vendored hits include `src/link_analysis.rs` and API docs for link analysis. |
| shortest_path | Yes | Local vendored hits include `src/iterators.rs` and path-related Rust modules. |
| community | Partial | Local vendored hits are weaker and include random graph/docs references; community detection is not the core reason to adopt. |

## Our use case fit

Good fit for M061+ scale tests where traversal, shortest path, and connected component latency dominate. The main caution is API mismatch and weaker community-algorithm evidence than igraph.

## Decision

**ADOPT** as a second pip-installable candidate for ADR-016 comparison. Rationale: installable via uv, Rust backend, strong traversal/path performance potential, and useful contrast with igraph.
