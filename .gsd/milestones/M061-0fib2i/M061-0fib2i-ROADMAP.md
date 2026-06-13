# M061-0fib2i: M060c Graph Library Alternatives Research and Applicability

**Vision:** Research graph library alternatives to NetworkX: benchmark pip-installable options (igraph, rustworkx) on our 4-layer graph + synthetic scale; evaluate not-adopted (graph-tool conda, PyG/DGL GNN, NetworkX-Temporal, GraphScope); emit ADR-016 with binding decision for our project.

## Slices

- [x] **S01: Benchmark pip-installable alternatives: igraph + rustworkx on our graph** `risk:low` `depends:[]`
  > After this: igraph + rustworkx installed, benchmarked on 9418-edge graph + synthetic 10k/100k graphs, comparison report emitted

- [x] **S02: Applicability matrix + ADR-016 (binding) + closeout** `risk:low` `depends:[S01]`
  > After this: Applicability matrix for 7 libraries, ADR-016 binding decision, decision doc for M061-M065

## Boundary Map

Not provided.
