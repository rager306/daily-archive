# prismAId profile for daily-archive

## Summary

prismAId is an open-source, protocol-bound toolkit for AI-assisted systematic literature reviews. Its strongest relevance to daily-archive is not autonomous discovery, but its reproducible workflow pattern: explicit screening rules, source acquisition tracking, document conversion, structured extraction, audit logs, and human review gates.

## Architecture and workflow

- **Positioning:** prismAId describes itself as “Open Science AI Tools for Systematic, Protocol-Based Literature Reviews,” using generative AI to streamline screening and extraction from scientific literature.
- **Core implementation:** the README states the core implementation is in Go, with Python, R, and Julia package access plus no-coding binaries for Windows, macOS, and Linux.
- **Workflow:** the official workflow is `Search -> Screen -> Download -> Convert -> Review`; the longer review workflow expands this into protocol design/registration, literature identification, acquisition, conversion, screening, configuration, analysis, and results processing.
- **Main tools:** Screening, Download, Convert, and Review.
- **Configuration model:** Review projects are TOML-configured with project settings, LLM settings, prompt components, and review-item schemas. The configurator generates those TOML files through a browser UI.

Sources:

- https://raw.githubusercontent.com/Open-and-Sustainable/prismAId/main/README.md
- https://prismaid.review/
- https://prismaid.review/review/review-workflow.html
- https://prismaid.review/tools/screening-tool.html
- https://prismaid.review/tools/download-tool.html
- https://prismaid.review/tools/convert-tool.html
- https://prismaid.review/tools/review-tool.html
- https://prismaid.review/review/review-configurator.html

## Source acquisition

- prismAId separates **literature identification** from **literature acquisition**: search/export happens upstream, then the Download tool acquires selected papers from URL lists or Zotero.
- Download inputs can be plain-text URL lists, CSV, or TSV. CSV/TSV handling includes column detection for URLs, DOIs, titles, authors, year, journal/source, and abstracts.
- For problematic web URLs that require JavaScript or authentication, prismAId prefers DOI fallback from CSV metadata, then Crossref lookup, then attempts the original URL.
- Download outputs preserve original metadata and add `downloaded`, `error_reason`, and `filename`, which is directly useful for daily-archive provenance and retry reporting.
- prismAId also uses Unpaywall fallback for open-access versions when direct downloads fail.

Sources:

- https://prismaid.review/review/review-workflow.html
- https://prismaid.review/tools/download-tool.html

## Provenance and citations

- **Repository:** https://github.com/Open-and-Sustainable/prismAId
- **Documentation:** https://prismaid.review
- **Software DOI:** Boero, R. (2024/2026), prismAId, Zenodo DOI: https://doi.org/10.5281/zenodo.11210796
- **JOSS article:** Boero, R. (2025), Journal of Open Source Software DOI: https://doi.org/10.21105/joss.07616
- **License:** GNU Affero General Public License v3.0 only.

Sources:

- https://raw.githubusercontent.com/Open-and-Sustainable/prismAId/main/LICENSE
- https://doi.org/10.5281/zenodo.11210796
- https://doi.org/10.21105/joss.07616

## Review and quality gates

- **Protocol-first gate:** prismAId requires review configuration before extraction; prompts define persona, task, expected result, definitions, examples, and failsafe behavior.
- **Schema gate:** Review items define keys and allowed values; the docs recommend exhaustive value lists for categorical data and empty values only when necessary for numerical/free-text fields.
- **Small-sample validation gate:** The workflow recommends testing with one paper, then a small batch, before running the full corpus.
- **Screening QA gate:** Screening best practices include manually checking excluded-item samples, tuning thresholds based on false positives/negatives, running multiple passes, and keeping originals.
- **Conversion QA gate:** Convert docs explicitly warn that PDF conversion can be imperfect and recommend manual checks of converted manuscripts, especially complex papers.
- **Consistency gate:** The Review tool supports duplication for prompt consistency testing; inconsistent duplicate results indicate unclear prompts.
- **Ensemble gate:** Multiple LLMs can be configured for ensemble review to support validation and uncertainty quantification.
- **Observability gate:** Screening and review tools support configurable log levels; screening can save runtime logs and a screening report.

Sources:

- https://prismaid.review/tools/review-tool.html
- https://prismaid.review/review/review-workflow.html
- https://prismaid.review/tools/screening-tool.html
- https://prismaid.review/tools/convert-tool.html

## Autonomy boundaries

prismAId is best understood as **protocol-bound automation**, not an autonomous scientist. The user defines search scope, review protocol, prompt schema, model/provider settings, and downstream interpretation.

It automates repetitive steps: deduplication, filtering, downloading, conversion, and structured extraction. It does not remove the need for human protocol design, source selection, conversion spot checks, exclusion review, or final synthesis.

The official workflow emphasizes PRISMA-aligned methodology, documentation, transparency, and including project configuration in appendices.

Source:

- https://prismaid.review/review/review-workflow.html

For daily-archive, the analogous boundary should be: agents may fetch, normalize, tag, summarize, and extract under explicit configs, but should not silently alter inclusion criteria, fabricate citations, or treat model outputs as ground truth without evidence gates.

## Failure modes

- **Document acquisition failures:** inaccessible URLs, JavaScript-only pages, authentication-required services, paywalls, bad DOIs, or failed open-access fallbacks. prismAId records download status and `error_reason`, which daily-archive should mirror.
- **Conversion failures:** PDFs lack semantic structure; multi-column layouts, figures/captions, equations, headers/footers, tables, footnotes, special characters, and scanned text can degrade extraction.
- **OCR cost/performance failures:** OCR can take 10 to 60 seconds per page and needs substantial memory for Tika; batch processing needs explicit resource planning.
- **Screening false positives/negatives:** deduplication thresholds, language detection, and article-type classification may be wrong; docs recommend manual sampling and threshold tuning.
- **LLM nondeterminism:** temperature zero does not guarantee identical answers, especially near token limits or with attention-window shifts.
- **Noise and training-data bias:** hidden information and model priors can bias extraction.
- **Rate-limit gaps:** TPM/RPM delays are supported, but daily request limits are not automatically managed and require manual monitoring.
- **Cost-estimate drift:** cost estimates are approximate and subject to provider changes.
- **Secret-handling risk:** Zotero/API-provider credentials are part of configs or environment; daily-archive should keep secrets out of logs, artifacts, and raw evidence files.
- **License contamination risk:** AGPL-3.0 matters if code reuse is considered; daily-archive should reuse patterns, not copy implementation, unless license obligations are explicitly accepted.

Sources:

- https://prismaid.review/tools/download-tool.html
- https://prismaid.review/tools/convert-tool.html
- https://prismaid.review/tools/screening-tool.html
- https://prismaid.review/review/review-workflow.html
- https://prismaid.review/tools/review-tool.html
- https://doi.org/10.5281/zenodo.11210796

## Reusable patterns for daily-archive

- **Protocol-as-config:** store extraction rules, allowed values, failsafes, model settings, and output schema in versioned config rather than embedding them in code.
- **Stage-separated pipeline:** keep acquisition, conversion, screening, extraction, and synthesis as separate stages with durable artifacts between them.
- **Source ledger:** preserve source URL/DOI, acquisition status, error reason, local filename/path, and transformation state for every item.
- **Evidence-first extraction:** require structured outputs with source citations/evidence spans; if evidence is absent or ambiguous, use configured failsafe values rather than forcing an answer.
- **Human-review queues:** route excluded papers, conversion anomalies, missing citations, and inconsistent model outputs to explicit review queues.
- **Small-to-large rollout:** validate new prompts/configs on one paper, then a small batch, then the daily corpus.
- **Conversion spot checks:** sample converted full text before extraction; flag zero-byte, very short, OCR-only, table-heavy, or equation-heavy documents.
- **Duplicate-run or ensemble gate for high-stakes fields:** use duplicate extraction or model ensembles only for fields where uncertainty materially affects downstream decisions.
- **Structured observability:** emit stage, item ID, source URL/DOI, decision, reason, model/provider, token/cost estimate, and retry count; never log secrets or raw copyrighted corpus content.
- **Output boundaries:** persist metadata, citations, extracted facts, short evidence snippets where legally appropriate, and derived summaries; avoid persisting raw third-party corpus text unless licensing permits it.

## Non-goals and safety risks

- **Non-goal:** copying prismAId code into daily-archive; the AGPL license makes direct reuse a legal/design decision, not an implementation shortcut.
- **Non-goal:** treating AI extraction as peer review or ground truth. It is a candidate extraction layer that requires source-backed validation.
- **Non-goal:** persisting raw copyrighted PDFs/full-text corpora in GSD artifacts or logs.
- **Non-goal:** letting models invent missing metadata, citations, inclusion rationales, or unsupported findings.
- **Safety risk:** chain-of-thought-style logs can expose unnecessary reasoning text and source content; daily-archive should prefer concise justification, evidence spans, source URLs, and deterministic decision fields over raw hidden reasoning.
- **Safety risk:** automated download can violate publisher terms or overload hosts if not rate-limited; use respectful concurrency, retries, and source-specific policies.
- **Safety risk:** model/provider APIs may receive manuscript text; sensitive or licensed content needs explicit policy before external model calls.
- **Safety risk:** daily request limits, budget caps, and provider-side policy changes are not fully handled by local pipeline code; add external budget/rate guards for production use.
