# S02: Bounded source acquisition for thirty paper scan — UAT

**Milestone:** M006-638rza
**Written:** 2026-05-19T17:08:20.876Z

# S02: Bounded source acquisition for thirty paper scan — UAT

## Smoke Test

Run the slice verification command and confirm it prints 3 tests passed, ruff clean, and an artifact guard with:

- `paper_count=30`
- `ready_for_markdown_scan_count=30`
- `still_missing_markdown_count=0`
- `available_pdf_count=8`
- `safety_flags_false=true`

## Expected Result

The 30-paper corpus is now ready for Markdown-based S03 deviation analysis.

## Not Proven

- PDF completeness: only 8/30 cached PDFs are available.
- Multimodal extraction/import readiness.
- Positive KG import readiness.
- Production LadybugDB write safety beyond no-write diagnostics.
