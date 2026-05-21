# M018 dependency security triage

## Verdict

`DEFER BROAD UPGRADE; ISOLATE DOCLING FALLBACK BEFORE NEW SOURCE-ACQUISITION RUNS`

The audit debt is real but not a main-CLI emergency. `torch` and `transformers` are vulnerable transitive packages pulled through `docling -> docling-ibm-models`. Project source does not directly import them, but Docling fallback is reachable from bounded source acquisition helpers that can process externally sourced arXiv PDFs.

## Evidence summary

| Evidence | Result |
|---|---|
| Direct runtime owner | `docling>=2.93.0` in `pyproject.toml` |
| Vulnerable packages | `torch 2.12.0`, `transformers 5.8.1` |
| Vulnerability count | 19 total: 11 torch, 8 transformers |
| pip-audit fix versions | none reported |
| Direct `torch` imports in source | 0 |
| Direct `transformers` imports in source | 0 |
| Direct `docling` import in source | 1 lazy import |
| Lazy import site | `src/arxiv_archive/md_converter.py:211` |
| Reachable helper path | `src/arxiv_archive/thirty_paper_source_scan.py:63` -> `MDConverter` |
| Main CLI exposure found | false |
| Validation-batch scan/preflight exposure | false |
| Source-acquisition helper exposure | true |
| Production KG import/write | still blocked |

## Finding 1: Vulnerable ML stack reachable through Docling fallback

**Severity:** Medium when source acquisition is run on external PDFs; Low/Dormant otherwise.

**Exploitability:** Local/operator-triggered helper path, not remote unauthenticated. The path can process externally sourced arXiv PDFs once a bounded source-acquisition run is launched.

**Location:**

```text
src/arxiv_archive/md_converter.py:211
src/arxiv_archive/md_converter.py:220
src/arxiv_archive/thirty_paper_source_scan.py:63
src/arxiv_archive/thirty_paper_source_scan.py:86
src/arxiv_archive/pdf_downloader.py:14
```

**Concrete path:**

```text
acquire_sources_for_manifest
  -> MDConverter.convert
    -> _try_arxiv2md
    -> _try_marker
      -> _try_docling
        -> from docling.document_converter import DocumentConverter
        -> DocumentConverter().convert(pdf_path)
```

`PDFDownloader.download` fetches external arXiv PDF bytes and validates content type/PDF magic before writing them locally. Docling then processes the local file. This is a real file-processing boundary, even though it is not exposed through the main validation-batch scan path.

**Recommendation:** Do not run new broad source-acquisition/Docling fallback batches until a follow-up isolation patch exists.

Recommended follow-up controls:

1. Add an explicit Docling fallback gate, disabled by default for unattended or broad runs.
2. Require a CLI/config flag or bounded allowlist to enable Docling fallback.
3. Preserve `fast_only=True` as the safe default where possible.
4. Emit a redacted diagnostic when Docling fallback is skipped because the gate is disabled.
5. Keep production KG import/write blocked.

## Finding 2: Blind upgrade is high risk and currently under-specified

**Severity:** Medium operational risk.

**Exploitability:** Not a security exploit by itself; this is remediation risk.

**Location:**

```text
pyproject.toml
uv.lock
```

`torch` and `transformers` versions are transitive under Docling, and `pip-audit` reported no fix versions. A blind upgrade of the ML stack may require coordinated Docling/docling-ibm-models/torch/torchvision/transformers compatibility work.

**Recommendation:** Defer broad package upgrade to a separate dependency-upgrade milestone only after the Docling fallback gate is added. If upgrading, batch it separately and verify PDF conversion fixtures and source acquisition behavior.

## Finding 3: No immediate MiniMax or KG safety regression

**Severity:** Informational.

M018 found no evidence that the vulnerable ML stack affects M017 MiniMax helpers, production KG import, or LadybugDB writes. Existing no-import/no-write/source-of-truth gates remain intact.

## Final recommendation by package

| Package | Recommendation | Rationale |
|---|---|---|
| torch 2.12.0 | Isolate now; defer upgrade | Vulnerable transitive package, no direct source import, reachable only through Docling fallback. pip-audit reported no fix version. |
| transformers 5.8.1 | Isolate now; defer upgrade | Same as torch; transitive via docling-ibm-models and only relevant when Docling fallback executes. |
| docling 2.93.0 | Keep for now but gate fallback | Direct dependency needed for current conversion strategy; broad removal would regress source repair capability. |
| docling-ibm-models 3.13.2 | No direct action now | Transitive owner of ML stack; handle with Docling upgrade/isolation milestone. |

## Proposed next GSD milestone

```text
Docling fallback safety gate
```

Scope:

- default Docling fallback disabled for unattended/broad source acquisition;
- explicit opt-in for bounded manual repair runs;
- redacted diagnostics for skipped fallback;
- tests covering gate off/on;
- no dependency upgrades in the same commit unless separately planned.

Optional later milestone:

```text
Docling and ML stack dependency upgrade spike
```

Only after the fallback gate exists.

## Non-findings considered

- No direct project source import of `torch` was found.
- No direct project source import of `transformers` was found.
- Validation-batch preflight and scan paths do not execute Docling conversion.
- M017 MiniMax helpers do not import or execute the vulnerable ML packages.
- Production KG import and LadybugDB writes remain blocked.

## Safety of this milestone

M018 performed triage only. No dependency files were changed. No raw PDFs, raw paper/chunk text, embeddings, vectors, model payloads, secrets, or raw audit JSON were persisted.
