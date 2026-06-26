# M172 Category Scope Decision

## Decision

Implement exactly three path-family categories:

1. `graph-readiness-evidence`
2. `source-asset-package`
3. `article-artifact-package`

These categories are approved only for exact reviewed source path families, not by generic target names.

## Approved exact scopes

| New category | Exact source path scope | Current categories affected | Why safe |
|---|---|---|---|
| `graph-readiness-evidence` | `src/research_graph/infrastructure/graph/readiness/` | caller-owned, run-scoped, append-log | Generated review/export/validation evidence under one subsystem path. |
| `source-asset-package` | `src/research_graph/infrastructure/papers/source_assets/registry.py` | caller-owned, run-scoped, append-log | Source asset package manifests, summaries, and diagnostics under one registry path. |
| `article-artifact-package` | `src/research_graph/cli/commands/article_artifacts.py`, `src/research_graph/infrastructure/papers/artifacts/` | caller-owned, run-scoped, append-log | Article artifact package manifests, summaries, diagnostics, and validation outputs. |

## Rejected for this milestone

| Candidate group | Reason |
|---|---|
| `other broad outputs` | Mixed paths; cannot safely reclassify as a group. |
| `cli outputs` | Mixed user-selected output targets; needs command-level review. |
| `parser replay outputs` | Viable later, but not needed for first expansion. |
| `source scan outputs` | Viable later, but source-scan intent should be reviewed separately. |
| `graph probe outputs` | Keep broad until graph-probe write roles are reviewed. |
| `repair benchmark outputs` | Keep broad until benchmark diagnostics are reviewed. |

## Implementation rule

The classifier may match exact path prefixes/files above. It must not match broad words such as `summary`, `manifest`, `events`, `diagnostics`, `state`, `index`, `catalog`, or `queue` globally.

## Test rule

Every new category needs:

- one positive test for an exact approved path;
- one fallback test showing an unapproved path with similar target words stays in the conservative existing category.
