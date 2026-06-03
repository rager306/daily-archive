# M029 Pipeline Architecture Audit Intake

## Requested URLs

| URL | Availability | Catalog status | Prior selection status | Next action |
|---|---:|---|---|---|
| `https://arxiv.org/abs/2507.19457` | HTTP 200 | already in `article_catalog` | not in M028 selection | Include existing catalog record in M029 analysis/replay scope. |
| `https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf` | HTTP 200, `application/pdf`, 202706 bytes | missing from `article_catalog` | not in M028 selection | Register metadata-only catalog record, then attempt controlled PDF acquisition/parser classification. |
| `https://arxiv.org/abs/2605.29548` | HTTP 200 | missing from `article_catalog` | not in M028 selection | Register metadata-only arXiv record, then attempt PDF acquisition/conversion-quality replay. |
| `https://arxiv.org/abs/2605.26099` | HTTP 200 | missing from `article_catalog` | already in M028 selection | Promote from M028 loader smoke selection into catalog-backed registration. |

## Direct answer

The requested refs are available for the next loading/analysis stage, but only one is already fully present in `article_catalog`:

- Already cataloged: `arxiv:2507.19457`.
- Already selected in M028 but missing from catalog: `arxiv:2605.26099`.
- New and missing from catalog: `arxiv:2605.29548`, `stanford:cs224n:gradient-notes`.

## Safety boundary

This intake does not claim source acquisition, parser readiness, chunk readiness, graph readiness, or production ingestion. It records a bounded input set for the next milestone and preserves fail-closed graph/import flags.

## Why this matters

The project currently has a split pipeline state: M028 loader smoke covers more refs than the reusable `article_catalog`. The next milestone should first reconcile catalog-backed corpus coverage, then replay acquisition, parser/conversion, chunking, and graph-readiness review from one continuous input contract.
