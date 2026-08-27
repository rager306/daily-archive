# M463 — GROBID-compatible parse plane (research)

Status: research (M463-snmgcd). Not an implementation ADR.
Binding: D203, R085, ADR-053, ADR-065, ADR-072, ADR-074, D127, D136.
Live evidence: `artifacts/research/m463/tei-inventory-2608.22118.json`,
`artifacts/research/m463/tei-inventory-2507.19457.json`.

## Decision frame (do not reopen)

| Lock | Rule |
|------|------|
| D203 | Analogue = provider-neutral **staged platform**, not a Java GROBID fork |
| R085 | Own admission, cache, backpressure, merge; GROBID starts as **one backend** |
| D127 | TEI/sidecar is candidate evidence, not graph truth; `import_eligible=false` |
| D136 | No LoRA / `.safetensors` under `data/policies`; SONA consult-only |
| ADR-050 | `kg-*` must not depend on `da-*`; scholarly TEI types stay product-side |
| ADR-065 | arXiv `idno` is the honest revision bind; do not invent `v1` |
| MEM681 | Native engines incrementally, gold-gated; no big-bang rewrite |

**Crate name:** `da-parse` (product). **Not** `kg-parse`.

---

## Wave 0 / S01 — TEI lift inventory (what GROBID already emitted)

Parser today (`GrobidParser::parse_pdf`): title, abstract, body, sections
(`path_id` from `<head n>`), citations + bibr anchors, tables, formulas,
raw `tei_xml`. HTTP: `isalive` + `processFulltextDocument` only.

### 2608.22118 (structural cache, 95 KB TEI)

| Family | TEI | ParsedArticle | Rank |
|--------|-----|---------------|------|
| Title / abstract / body | present | lifted | done |
| Sections + `path_id` from `<head n>` | 37 heads / 0 nested `div n` | 83/83 path_id | done (MEM654: hierarchy is lexical) |
| Tables (`figure type=table`) | 4 | 4 | done |
| Formulas | 0 | 0 + typed missingness | done (extractor idle) |
| Citations `biblStruct` | 15 | 14 (6 DOI, 2 arXiv, 18 anchors / 23 bibr) | partial |
| Header authors | **1** (`Gregory Druck`) | **no field** | **P0** |
| arXiv idno | `arXiv:2608.22118v1[cs.CL]` | not a ParsedArticle field | **P0** (ADR-065 scans `tei_xml` later) |
| Non-table `<figure>` + `<graphic>` | **48 / 28** | not lifted | **P0** |
| In-text `ref type=figure\|table` | 18 + 2 | not lifted | P1 |
| `ptr` URLs | 14 | not lifted | P1 |
| Notes | 2 | not lifted | P2 |
| Affiliation / funder / keywords | 0 / 0 / 0 | — | skip until TEI has them |
| Nested `<div n>` rewrite | 0 | N/A | **skip** |

### 2507.19457 gold TEI (403 KB; tests still pin this file)

| Family | TEI | Lift today | Rank |
|--------|-----|------------|------|
| Header authors | **21** | no field | **P0** |
| arXiv idno | `arXiv:2507.19457v2[cs.CL]` | string-scan at materialize | **P0** |
| Non-table figures | **22** (1 graphic) | not lifted | **P0** |
| Tables | 3 | extracted (test pin) | done |
| Formulas | **24** | extracted (test pin) | P2 materialize to graph |
| `biblStruct` / bibr | 81 / 123 | citations extracted | P1 authors-in-cite |
| figure/table refs | 24 / 5 | not lifted | P1 |
| `ptr` | 66 | not lifted | P1 |
| Notes | 116 | not lifted | P2 |
| `div n` | 0 | — | skip nested-div |

### P0 / P1 / P2 / skip

**P0 (data already in TEI, parser drops it)**

1. Lift `idno type="arXiv"` into `ParsedArticle` (revision string). Unblocks
   `versionless_source` without inventing v1. ADR-065 already parses the same
   string from `tei_xml`; the parser should own the field.
2. Lift header `<author><persName>` into `ParsedArticle.authors`. Gold TEI has
   21; live 2608.22118 has 1 (GROBID miss of the second author — typed
   missingness, not a parser bug).
3. Lift non-table `<figure xml:id>` + caption + `graphic coords` as a
   **candidate** figure index. Not graph truth. pdffigures2 stays the crop
   detector until gold conservation is measured.

**P1**

4. In-text `ref type="figure"|"table"` using the existing bibr-anchor machine.
5. Citation `persName` / venue inside `CitationEntry` (today: raw/doi/arxiv/title).
6. `ptr` URLs as reproducibility flags (code/data), not graph nodes.

**P2**

7. Notes. 8. Formula nodes into the graph (extractor exists). 9. Table-cell
   extraction surface (ADR-071;  cells already in `ExtractedTable`).

**Skip this programme**

- Nested `<div>` rewrite (real TEI is flat; `head n` already drives `path_id`).
- DeLFT / SciBERT-CRF GPU. CRF F1 0.869 is enough.
- TEI as graph truth. Patent APIs. Training APIs. Java source fork.
- MinerU/Docling/Marker as a TEI replacement (MD/JSON, lossy vs bibliography).

---

## Wave 1 / S02 — GROBID REST subset (compatible facade)

Official surface (GROBID 0.9 docs, 2026-08): PDF→TEI, raw-text helpers,
PDF annotation, patents, training. Errors: HTTP 200/204/400/500/503 plus
typed codes `BAD_INPUT_DATA`, `NO_BLOCKS`, `TOO_MANY_BLOCKS`,
`TOO_MANY_TOKENS`, `TIMEOUT`, `TAGGING_ERROR`, `PARSING_ERROR`,
`PDFALTO_CONVERSION_FAILURE`, `GENERAL`. Consolidation: `0|1|2` and header `3`.
`teiCoordinates` is a JSON array of groups. Parallel mode: 503 = pool busy,
client must wait (~2s header, longer fulltext).

### Must (client + future facade)

| Endpoint | Today | Facade |
|----------|-------|--------|
| `GET /api/isalive` | used | liveness |
| `GET /api/version` | unused | version/revision |
| `GET /api/health` | unused | readiness + pool metrics |
| `POST /api/processFulltextDocument` | used | default parse |
| `POST /api/processHeaderDocument` | unused | cheap header/idno/authors |
| `POST /api/processReferences` | unused | bibliography-only salvage |

Multipart must: `input` (PDF), `consolidateHeader`, `consolidateCitations`
(`0|1|2`, header also `3`), `teiCoordinates`, `start`/`end`.
Should: `includeRawAffiliations`, `includeRawCopyrights`, `consolidateFunders`.
Fingerprint (D204) already includes profile + consolidate + coordinates;
new fields must join the fingerprint or they are forbidden.

### Should (client only, not first facade)

- `POST /api/processCitation` and `processCitationList` — salvage raw biblio
  strings (AnyStyle-class). Useful after 14/36-page cite under-extract.
- `POST /api/referenceAnnotations` — PDF coords for in-text callouts.
- Admin `:8071/metrics/prometheus` — scrape, do not reimplement.

### Wont

Patents (`processCitationPatent*`), training (`modelTraining`…),
`annotatePDF` binary rewrite, `debugMode` as a product API.
Markdown/JSON converters live **client-side** in official Python; TEI stays
the server truth. Our `ParsedArticle` is the product projection, not CORD-19.

### Compatibility rule

A GROBID-compatible service is a **subset** that speaks the same paths,
multipart names, consolidate enum, and 503 semantics. It may route
`processFulltextDocument` to Java GROBID **or** (later) a native backend
that still returns TEI. It must not invent endpoints or silently drop
`teiCoordinates` from the cache key.

---

## Wave 2 / S03 — Native clients and engines (C / Rust / Go)

There is **no** 1:1 TEI+citation-context+consolidation clone in Rust or C.

| Name | Lang | Role | TEI | Citation context | Use for us |
|------|------|------|-----|------------------|------------|
| grobidOrg/grobid 0.9 CRF | Java | engine (default backend) | yes | yes | keep |
| grobid-client-python | Python | official batch client | consumes TEI | n/a | pattern: concurrency + 503 retry |
| grobid-client-java / node | Java / JS | official clients | consumes | n/a | same |
| **miku/grobidclient v0.2.7** | **Go** | third-party HTTP client | consumes | n/a | closest native-client analogue |
| crates.io `grobid*` | Rust | **none mature** | — | — | we write `da-parse` |
| pdfalto (GROBID stack) | C++ | PDF→XML blocks | no | no | already inside GROBID; do not fork |
| Wapiti / CRF++ | C++ | sequence labeler | n/a | n/a | do not re-host models |
| CERMINE | Java | engine, JATS | no (JATS) | weak | backend candidate later |
| Science Parse / s2orc-doc2json | Java/Python | JSON | no | mentions | schema inspiration only |
| AnyStyle / ParsCit | Ruby/CRF | ref-string parse | no | no | P1 citation salvage backend |
| pdffigures2 | Scala/Java | figures/tables | no | no | already in figure pipeline |
| MinerU / Docling / Marker / olmOCR / Nougat | Python | MD/JSON, OCR, math | **no** | no | optional **body/math/OCR** backends after gold gate |
| PyMuPDF / pdfplumber | C/Python | coords + text | no | no | layout boxes for XY-Cut (ADR-072) |
| Camelot / Tabula | Python | tables | no | no | not needed; TEI tables already lift |

**Recommendation:** HTTP client + TEI lift in Rust (`da-parse`). Keep Java
GROBID as the scholarly bibliography engine. Optionally add Go-client
semantics (bounded pool, 503 backoff) rather than a Go binary. Modern MD
engines may join the **staged platform** as replaceable backends for
layout/OCR, never as graph truth and never as a silent TEI substitute.

---

## Wave 3 / S04 — Parse-plane metrics (not entity F1)

`eval_batch` entity F1 (corpus ~0.87) can stay green while authors=0 and
48 figures are dropped. Parse quality is **conservation versus TEI** plus
identifier recovery. Gold for lift tests is the TEI fixture, not the
extractor under test (ADR-051: no common-mode oracle).

| Metric | Formula | Gold | Fail-closed |
|--------|---------|------|-------------|
| Header author recall | `lifted_header_authors / tei_header_authors` | TEI `<fileDesc><author>` | empty authors → typed missingness, not "0 authors paper" |
| arXiv idno bind rate | papers with lifted `vN` / papers whose TEI has `idno type=arXiv` | TEI idno | missing idno → `versionless_source`, no invented v1 |
| Citation identifier rate | cites with doi or arxiv_id / biblStruct | TEI listBibl | under-extract stays visible (2608: 14 vs expected ~30+) |
| bibr anchor recall | lifted anchors / `ref type=bibr` | TEI body | unresolved target → drop with typed missingness (already) |
| Figure conservation | lifted nontable figures / TEI nontable figures | TEI `<figure>` minus tables | 2608 baseline **0/48** |
| Table conservation | lifted tables / TEI table-figures | TEI | 2608 **4/4** |
| Formula conservation | lifted formulas / TEI formulas | TEI | 2507 **24/24** extractor; graph materialize separate |
| ptr URL recovery | lifted ptr / TEI ptr | TEI | optional P1 |
| HTTP health | 204 / 503 / TIMEOUT / NO_BLOCKS rates | sidecar logs | circuit-breaker input (R085) |
| Profile delta | structural vs rich on same PDF | paired cache | rich may not run on corpus (Crossref 429) |

**Not a parse gate:** entity Micro-F1, graph node counts, screenshot UAT.

Baseline to beat after P0 lift (2608.22118 cache): authors 0→≥1, idno
field present (`v1`), figures 0→48 candidates, tables stay 4/4.

---

## Wave 4 / S05 — FSM / SONA in the extract cycle

Existing path (ADR-074, `parse_grammar.rs`):

```
ArticleFeatures → grammar_route (WASM or native thresholds)
  → GrammarRoute {page_outline, xy_cut, grobid_structural}
  → grammar_profile (DATA profile_map)
  → GrobidParseStage | HTML | XY-Cut intent
```

Fail-open: host Load/Instantiate/Io → native; unknown route →
`default_profile: arxiv_pdf_dot`. XY-Cut has **no CLI** yet: stamp intent,
keep GROBID parse (MEM930). `policy_hash` is already stamped.

| Decision point | SONA / WASM may | Must not |
|----------------|-----------------|----------|
| `grammar_route` | pick backend class from pages/bookmarks/text quality | replace CRF models |
| `batch_admission` | skip empty / defer low text quality | drop TEI on cache hit |
| `retry_policy` | 503 backoff, timeout budget | retry until Crossref 429 succeeds on structural path |
| `concurrency_tune` | pool size vs GROBID engine idle | hide 503 |
| `summary_depth` | downstream of parse | parse TEI |

**SONA three-loop** (ADR-074 W1–W4) is allowed later as **routing/admission
weights**, still behind D136: no auto-promote, dual F1 + same-n + val_gap
if a learned policy ships. Parse extract cycle stays deterministic TEI lift.
Critic repair / NegativeMemory consult **after** parse, not inside CRF.

Do not invent a `sona-policy` CLI as the only consult path (MEM930).
`parse-pdf` / `GrobidParseStage` must consult `grammar_route` fail-open.

---

## Wave 5 / S06 — ADR-075 and `da-parse` crate (proposed)

**ADR number:** 075 (next after 074). Status to set: Proposed.

### Choice

1. New product crate `crates/da-parse`:
   - HTTP GROBID client (isalive/health/version/fulltext/header/references)
   - TEI lift → `ParsedArticle` (+ authors, idno, figures)
   - typed errors matching GROBID HTTP/error codes
   - optional later: axum facade exposing the **must** subset
2. `da-adapters::GrobidParser` becomes a thin re-export or wrapper.
3. `da-application` keeps FSM, cache identity, merge, ACAP admission.
4. `da-cli` dual-compile: no `crate::background_tasks` from bin-shared files.
5. Reject `kg-parse`: TEI/ParsedArticle are scholarly-product types.

### Facade shape (later implementation milestone, not M463)

```
da parse-serve --port 8072   # not 8070 (Java GROBID), not 16379 (graph)
  GET  /api/isalive|/api/health|/api/version
  POST /api/processFulltextDocument   → backend GROBID or native
  POST /api/processHeaderDocument
  POST /api/processReferences
```

Default backend = Java GROBID `:8070`. Facade is a **compatible subset**,
not a replica of patents/training. Cache key = D204 fingerprint. 503 from
upstream is 503 to the caller (honest). D127: facade output is not import.

### Follow-up implementation milestones (not this research unit)

| After ADR-075 accepted | Work |
|------------------------|------|
| M464 (impl P0 lift) | idno + authors + figure index in parser; tests on both TEIs |
| M465 (crate extract) | move `grobid_parser.rs` (~3500 lines) into `da-parse` |
| M466 (metrics gate) | conservation scores in `eval` example; CI on fixtures |
| M467 (header/refs endpoints) | client methods; optional facade |
| M468+ | AnyStyle salvage, XY-Cut grammar CLI, MD backend behind gold |

---

## What this programme is not

- Not a GROBID Java rewrite in Rust/C.
- Not MinerU-as-default.
- Not opening D127.
- Not a 295-paper graph wave.
- Not LoRA on CRF.
- Not editing `try_parse_arxiv_locator` (GitNexus CRITICAL; policy stays sibling).

## Next executable unit

M463/S01 T01–T03: inventories are already on disk; remaining work is the
ranked matrix (this file) plus GSD RESEARCH projection, then S02 REST table
as a sketch slice. Implementation of P0 lift waits for ADR-075 accept
**or** a user yes to lift inside `da-adapters` first (smaller blast radius).
