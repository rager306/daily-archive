# M060b NetworkX Graph Validation

This report is read-only. Production import is not authorized. Graph writes are disabled.
External network access is disabled. LLM calls are disabled. Fact promotion is disabled.

Overall status: `pass_with_warnings`

## Summary

- Passed checks: 6
- Warnings: 1
- Failed checks: 0

## Checks

| Check | Status | Message |
|---|---|---|
| `safety_defaults_explicit_false` | `PASS` | All five safety defaults are explicit and false. |
| `loopback_bind_host` | `PASS` | Loopback bind host is 127.0.0.1. |
| `citation_orphans` | `PASS` | No orphan nodes were found in the citation layer. |
| `duplicate_edges_per_layer` | `PASS` | No duplicate artifact edges were found within any layer. |
| `self_loops` | `PASS` | No artifact-level self-loops were found. |
| `content_sha256_sample` | `PASS` | content_sha256 matches actual file bytes for 5 sampled PDFs. |
| `paper_pair_layer_separation` | `WARN` | Some paper pairs appear in multiple layers; this is allowed but flagged. |

## Flagged Layer Separation

- `2204.01691` -> `2207.05608` appears in layers: citation, table_similarity
- `2601.05808` -> `2602.10090` appears in layers: figure_similarity_v1, figure_similarity_v2
