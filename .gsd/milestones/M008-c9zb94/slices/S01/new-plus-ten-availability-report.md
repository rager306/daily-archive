# New plus ten availability report

Overlap with M006: 0
Markdown available before S02: 1/10
PDF available before S02: 1/10

## Selected papers

- `1701.00001` — markdown=True, pdf=True, risk_tags=markdown_available, pdf_available
- `2001.00234v1` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf
- `2001.00236v1` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf
- `2001.00238v2` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf
- `2001.00248v2` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf
- `2001.00254v1` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf
- `2001.00258v2` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf
- `2001.00265v1` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf
- `2001.00267v1` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf
- `2001.00271v1` — markdown=False, pdf=False, risk_tags=missing_markdown, missing_pdf

## Expected S02 behavior

S02 should run validation-batch init/preflight. Because only 1/10 papers has existing Markdown, bounded acquisition/repair is likely required before scan. If acquisition cannot make the batch source-ready, S02 must block scan explicitly rather than faking readiness.
