# Continue — M423 implemented (pending GSD seal)

## Last action

M423-f31d3r S01+S02 implemented (code green; GSD skip-delivered closeout next):

- S01 domain: `TriageModeDiff` (typed per-field agreement, Eq; confidence
  is a 0.05 diagnostic band `TRIAGE_CONFIDENCE_EPSILON`). da-domain pdf
  tests 15 passed.
- S02 canary: `classify_sync_with_options` (product `classify_sync` stays
  Full) + `examples/pdf_triage_canary.rs` (required-features
  pdf-classifier; exit 0 agree / 2 divergence / 1 error; page scope is
  canary-only with an explicit warning).

**Measured on hub PDF 2507.19457** (MEM916): Full = OCR pages 69+84
suspected_garbled_text + encoding_issues=true, 5189ms. DetectOnly = zero
OCR pages, no encoding issues, 309ms. Detect-only is 17x faster but blind
to garbled text; product triage stays Full as a measured decision.
Page-scoped detect-only also returns an empty OCR list — page scope can
never feed the admission ratio.

Governor pins: `pdf_triage_canary` (example) + `pdf_triage_options_surface`
(adapter). hub_ocr_probe 2/2 green (regression intact).

## Next action (priority)

1. GSD skip-delivered closeout for M423 (UAT + sha256 validate + complete).
   Note: pre-commit runs before the stale exec markers are superseded —
   the `gsd_exec_false_success_evidence` flag (ids 03dc3c63, 71b30a3a from
   red-first TDD runs piped through tail/echo) clears once the green state
   is committed and a fresh preflight runs after the commit.
2. Wave 2 pdf-inspector followUps are now exhausted honestly: TSR needs an
   independent cell detector; ToUnicode repair has no pdf-inspector path;
   detect-only is measured blind. Next candidates: P1 remote GraphStore is
   DONE (M421); consider ToUnicode/markdown repair upstream or new debt.

## Do not

Unlock D127. Switch product triage to detect_only (measured blind, MEM916).
Feed a page-scoped OCR list into garbled_ratio. Retry TSR without an
independent cell detector. Soak-wakeup loops. Feature-gated code must pass
default-build clippy. Governor `forbidden_patterns` are substring scans.
Pipe cargo test through tail/echo — it masks non-zero exits and trips
`gsd_exec_false_success_evidence`; capture with `; echo EXIT:$?` on the
same line, never after a pipe.
