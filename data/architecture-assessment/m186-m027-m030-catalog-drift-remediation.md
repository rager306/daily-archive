# M186 M027 and M030 Catalog Drift Remediation

## Verdict

**PASS: data-only catalog remediation restored M027/M030 baseline consistency.**

## Changes applied

- Added fail-closed metadata-only `article.json` records for six canonical directories that already contained local PDFs.
- Removed stale duplicate directory `data/article_catalog/article_catalog/arxiv/cs-lg/2507.19457` after confirming its PDF hash matched the canonical `cs-cl` record.
- Regenerated both catalog index files from the canonical tree:
  - `data/article_catalog/index.json`
  - `data/article_catalog/article_catalog/index.json`
- Ensured the Stanford root index entry includes `normalized_identity: stanford:cs224n:gradient-notes`.

## Safety posture

The new records do not claim parser readiness, chunk readiness, graph readiness, production persistence, or network fetches. Safety flags remain fail-closed.

## Immediate proof

- Catalog consistency script passed.
- Full M027 mixed-source catalog tests passed: 13 passed.
- M030 requested-ref intake validation passed.
