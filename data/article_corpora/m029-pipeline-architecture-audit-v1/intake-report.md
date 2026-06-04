# M029 Pipeline Architecture Audit Intake

## Requested URLs

| URL | Availability | Catalog status | Prior selection status | Next action |
|---|---:|---|---|---|
| `https://arxiv.org/abs/2507.19457` | HTTP 200 | already in `article_catalog` | not in M028 selection | Include existing catalog record in M029 analysis/replay scope. |
| `https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf` | HTTP 200, `application/pdf`, 202706 bytes | already in `article_catalog` | not in M028 selection | Use the metadata-only `stanford/cs224n/gradient-notes` record before any future PDF acquisition or parser classification. |
| `https://arxiv.org/abs/2605.29548` | HTTP 200 | already in `article_catalog` | not in M028 selection | Use the metadata-only `arxiv/mixed-source/2605.29548` record before any future PDF acquisition or conversion-quality replay. |
| `https://arxiv.org/abs/2605.26099` | HTTP 200 | typed catalog blocker | already in M028 selection | Register an article.json-backed metadata record before treating this M028-selected identity as cataloged. |

## Direct answer

The requested refs remain a bounded, local-only intake baseline for `m029-pipeline-architecture-audit-v1`. three identities are now represented in `article_catalog`:

- Existing catalog record from the prior baseline: `arxiv:2507.19457`.
- Metadata-only M031 registration: `stanford:cs224n:gradient-notes`.
- Metadata-only M031 registration: `arxiv:2605.29548`.

one identity is an explicit typed catalog blocker rather than a silent missing or unsafe catalog claim:

- `arxiv:2605.26099` is present in the M028 selection, but the shared `article_catalog` index has no article.json-backed row after `catalog_record_present:false` placeholders were pruned. It must be registered as a real metadata-only catalog record before future work treats it as cataloged.

## Safety boundary

This intake does not claim source acquisition, parser readiness, chunk readiness, graph readiness, production ingestion, or LadybugDB persistence. It records a bounded input set and preserves fail-closed graph/import flags for all four refs.

## Why this matters

The stale M030 S01 assessment is superseded by the current M031 evidence: the two remaining missing M030 refs now have metadata-only catalog records, and the only unresolved four-ref baseline item is represented as a typed blocker with a concrete catalog remediation path. This closes stale missing-status drift for the remediated refs without making source-loader, parser, chunk, graph, production, or LadybugDB claims.
