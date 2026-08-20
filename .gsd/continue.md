# Continue — M422 implemented (pending GSD seal)

## Last action

M422-w6oqvh S01+S02 implemented (code green; GSD skip-delivered closeout next):

- S01: `PageTextMissing` + `page_text_missingness` on `ParsedArticle`
  (`serde default` = unset). Helper `page_text_missingness_from`.
  GROBID/HTML leave the field unset. No ToUnicode repair invented.
- S02: ingest stamps from `PdfClassification` after parse. `None` without
  classifier; `Some(empty)` classified-clean; `Some(pages)` mixed OCR.
  `body_text` unchanged. Governor `page_text_missingness_stamp`.

## Next action (priority)

1. GSD skip-delivered closeout for M422.
2. Then remaining Wave 2-b: page-scope/detect-only for canaries, **or**
   leave ToUnicode repair deferred (no honest repair path in pdf-inspector).

## Do not

Unlock D127. Invent ToUnicode repair. Fake HOT DirectGraphStore over RESP.
Retry TSR without an independent cell detector. Soak-wakeup loops.
Feature-gated code must pass default-build clippy. Governor
`forbidden_patterns` are substring scans. GSD BROWSER_REQUIREMENT_RE
treats hyphen as a word boundary; UAT `commandOrTool` must contain
`gsd_uat_exec`.
