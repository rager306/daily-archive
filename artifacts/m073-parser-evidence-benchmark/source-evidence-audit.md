# M073 Source Evidence Audit

## Scope

This audit checks M072 train/validation fixture refs against the canonical article catalog and existing parser manifest artifacts. It records paths, availability, and safety flags only; it does not persist raw article text, prompts, embeddings, model payloads, graph writes, or promotion output.

## Summary

- `case_count`: 9
- `article_json_available`: 9
- `canonical_pdf_available`: 7
- `parser_manifest_available`: 4
- `missing_parser_manifest`: 5

## Records

| Split | arXiv ID | canonical_pdf | parser_artifact_count | evidence_status |
|---|---|---:|---:|---|
| train | `1206.6423` | true | 5 | `parser_manifest_available` |
| train | `1409.0473` | true | 5 | `parser_manifest_available` |
| validation | `1606.02447` | true | 5 | `parser_manifest_available` |
| validation | `1611.04230` | true | 5 | `parser_manifest_available` |
| train | `2108.12409` | true | 0 | `canonical_pdf_only` |
| validation | `2109.10862` | true | 0 | `canonical_pdf_only` |
| train | `2507.19457` | true | 0 | `canonical_pdf_only` |
| train | `2511.20639` | false | 0 | `missing_canonical_pdf_and_parser_manifest` |
| train | `2605.18211` | false | 0 | `missing_canonical_pdf_and_parser_manifest` |

## Parser artifact notes

### 1206.6423
- canonical_pdf: `data/article_catalog/article_catalog/arxiv/cs-cl/1206.6423/source/1206.6423.pdf`
- parser_artifact: `artifacts/m061-2hop/anchor-2207.05608/parsing/paper-manifests/1206.6423.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2401.04016/parsing/paper-manifests/1206.6423.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2505.19443/parsing/paper-manifests/1206.6423.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2510.12157/parsing/paper-manifests/1206.6423.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2605.18747/parsing/paper-manifests/1206.6423.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor

### 1409.0473
- canonical_pdf: `data/article_catalog/article_catalog/arxiv/cs-cl/1409.0473/source/1409.0473.pdf`
- parser_artifact: `artifacts/m061-2hop/anchor-2207.05608/parsing/paper-manifests/1409.0473.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2401.04016/parsing/paper-manifests/1409.0473.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2505.19443/parsing/paper-manifests/1409.0473.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2510.12157/parsing/paper-manifests/1409.0473.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2605.18747/parsing/paper-manifests/1409.0473.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor

### 1606.02447
- canonical_pdf: `data/article_catalog/article_catalog/arxiv/cs-cl/1606.02447/source/1606.02447.pdf`
- parser_artifact: `artifacts/m061-2hop/anchor-2207.05608/parsing/paper-manifests/1606.02447.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2401.04016/parsing/paper-manifests/1606.02447.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2505.19443/parsing/paper-manifests/1606.02447.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2510.12157/parsing/paper-manifests/1606.02447.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2605.18747/parsing/paper-manifests/1606.02447.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor

### 1611.04230
- canonical_pdf: `data/article_catalog/article_catalog/arxiv/cs-cl/1611.04230/source/1611.04230.pdf`
- parser_artifact: `artifacts/m061-2hop/anchor-2207.05608/parsing/paper-manifests/1611.04230.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2401.04016/parsing/paper-manifests/1611.04230.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2505.19443/parsing/paper-manifests/1611.04230.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2510.12157/parsing/paper-manifests/1611.04230.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor
- parser_artifact: `artifacts/m061-2hop/anchor-2605.18747/parsing/paper-manifests/1611.04230.json`; diagnostic_only=True; graph_writes_authorized=False; fact_promotion_authorized=False; parsers=grobid-fulltext,opendataloader,plotextractor

### 2108.12409
- canonical_pdf: `data/article_catalog/article_catalog/arxiv/cs-cl/2108.12409/source/2108.12409.pdf`
- parser_artifact: missing

### 2109.10862
- canonical_pdf: `data/article_catalog/article_catalog/arxiv/cs-cl/2109.10862/source/2109.10862.pdf`
- parser_artifact: missing

### 2507.19457
- canonical_pdf: `data/article_catalog/article_catalog/arxiv/cs-lg/2507.19457/source/2507.19457.pdf`
- parser_artifact: missing

### 2511.20639
- canonical_pdf: missing
- parser_artifact: missing

### 2605.18211
- canonical_pdf: missing
- parser_artifact: missing

## S02 handoff

- Prefer existing parser manifest paths as evidence refs where available.
- For refs without parser manifests, preserve explicit `missing_parser_manifest` diagnostics instead of fabricating evidence paths.
- Do not copy PDF body text or parser body text into fixtures.
- Keep `write_eligibility=false` and `promotion_eligibility=false` in queue metadata verification.
