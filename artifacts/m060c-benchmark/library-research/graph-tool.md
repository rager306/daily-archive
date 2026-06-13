# graph-tool rejection note

## Status

**NOT_VENDORED**. graph-tool was not vendored because git access was blocked; this was deferred per M048. No GitNexus repo is available for source-level verification in this milestone.

## Decision

**DEFER**. Rationale: graph-tool may offer major speedups, but conda/system-package friction is greater than the expected benefit at our current 10k-100k edge scale.

## Recommendation

Revisit graph-tool only if the M061+ scale tests show that pip-installable igraph/rustworkx and NetworkX cannot meet latency needs, or if the project requires the reported 40-250x speedup enough to justify conda/runtime complexity.
