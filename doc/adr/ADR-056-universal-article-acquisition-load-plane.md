# ADR-056: Universal Article Acquisition & Load Plane

**Status:** Accepted  
**Date:** 2026-08-10  
**Accepted:** 2026-08-10 (user confirm: architecture summary correct)  
**Deciders:** collaborative  
**Related:**
- ADR-037 / ADR-041 (hexagonal onion, HOT ingest path)
- ADR-039 (lifecycle tags, staged validation, fail-closed import)
- ADR-040 (Samyama sole store; no second persistence for article state)
- ADR-043 (multi-source corpus; source_code / domain from day one)
- ADR-049 (pipeline DSL — load stages must be composable, not CLI-only)
- ADR-050 (ontology / source codes as DATA)
- ADR-051 (verification contours for ports, application, adapters)
- ADR-052 (structured logging — lifecycle events with semantic identity)
- ADR-053 (article preparation layers after structure exists)
- ADR-055 (ACAP FSM + **cache contract**: semantic key ≠ retry timing; graph mutation outside cacheable stages)
- Existing ports/types to **reuse, not fork**:
  - `PipelineCache` / `StageCacheKey` / `SourceIdentity` (`da-ports::pipeline_cache`)
  - `PipelineStageExecutor` / `StageContract` (`da-ports::pipeline_stage`)
  - `FilePipelineCache` (`da-adapters`)
  - `TaskType` / `PendingTask` / `GraphScheduler` / `SchedulerTask` nodes (graph queue, not JSONL)
- D127 (import_eligible remains false until explicit user yes)
- D136 (DSPy/LLM extraction locked until eval fixtures verified)
- Operator data: `data/source_codes.yaml`, canonical catalog under `data/article_catalog/`

**Binding Level:** binding once Accepted  
**Revisable:** yes, with milestone evidence — not by CLI convenience hacks

---

## 0. One-line Decision

> **Article load is a universal application use case** (`ArticleLoadUseCase`) over
> provider-agnostic **acquisition ports** and the existing ingest/extract/graph
> ports. **arXiv is one acquisition provider**, not the load architecture.
> Catalog materialization, multi-representation assets (PDF/HTML/TeX/…),
> checksum identity, fail-closed network, D127, ACAP source-layer evidence,
> **content-addressed stage cache** (reuse `PipelineCache` / `SourceIdentity`),
> **graph-backed work queue** (reuse `SchedulerTask` / `TaskType`, not a new
> queue product), and durable load reports are in-scope from day one so the
> operator path does not have to be redesigned when the second provider appears.

---

## 1. Context — why this ADR exists

### 1.1 Symptom

Operators currently cannot run a single, trustworthy command:

```text
locator (URL / id / catalog key / local path)
  → ensure catalog assets
  → parse + embed + materialize
  → optional extract
  → durable snapshot + load report
```

What exists instead:

| Surface | Role today | Gap |
|---------|------------|-----|
| `da ingest --pdf PATH --id ID` | single PDF path | no acquisition, no catalog ensure |
| `da batch-ingest --ids …` | multi-PDF HOT path | discovers PDF by ad-hoc `find`, not via port |
| ~~`scripts/download_arxiv_to_catalog.py`~~ (archived M386 S04) | ~~arXiv → catalog~~ | **Replaced** by `da acquire-arxiv` (Rust); archived to `archive/python-era-cold-s04/` (D237) |
| `CatalogInventoryPort` / `CatalogMutationPort` | inventory + atomic write | no “ensure missing assets” use case |
| `EnrichUseCase` (OpenAlex) | post-materialization metadata | not acquisition of primary content |
| draft `load_arxiv.rs` | CLI-shaped arXiv script wrapper | **anti-pattern** — provider-rooted CLI, not a plane |

Result: “loading” is performed as a **manual sequence of scripts + CLI**, fails
silently at boundaries (missing PDF, wrong category, network policy, partial
ingest without report), and cannot be extended to textbook HTML / local drops /
DOI-first providers without another one-off.

### 1.2 Constraints already binding

1. **Onion / hexagonal (ADR-037, ADR-050):**  
   `da-domain ← da-ports ← da-application ← da-cli`;  
   `da-adapters` implements ports; application must not depend on adapter paths.
2. **Samyama sole state store (ADR-040):** load does not invent a second DB for
   “what was loaded”. Durable evidence may live as **artifacts** (JSON/snapshot)
   and catalog files; graph truth stays in Samyama.
3. **D127:** `import_eligible=false` on every write unless the user explicitly
   unlocks for that run. Load never implies promotion.
4. **ACAP (ADR-055):** source acquisition is the first FSM layer
   (`source_registered` / `source_verified`). Load must emit assurance-relevant
   evidence (checksums, missingness, network authorization), not only “exit 0”.
5. **Multi-source corpus (ADR-043, `source_codes.yaml`):** arXiv, textbook,
   Stanford, OpenAlex, … are **source codes**, not separate products.
6. **Catalog is canonical for primary content (KNOWLEDGE rule #1 / M061):**  
   downloaded articles end in  
   `data/article_catalog/article_catalog/<source>/<topic>/<id>/…`  
   with checksum-bound evidence. Isolation under `artifacts/` alone is a defect.
7. **Versionless identity with versioned provenance:** modern arXiv ids are
   versionless keys; versioned local assets may exist as provenance when content
   differs; bare metadata is preferred for lookup (project memory).
8. **Network is on by default (M462):** missing catalog assets are fetched.
   `--disallow-network` / `DA_LOAD_DISALLOW_NETWORK=1` is the CI/replay opt-out.
   Catalog hits must still work with `allow_network=false`.
9. **Pipeline composability (ADR-049):** load stages must be invocable as pure
   application steps (and later as pipeline nodes), not only as CLI glue.

### 1.3 Non-goals of *this* ADR

- Opening D127 / import eligibility.
- Replacing GROBID / changing extraction algorithms.
- Auto-promoting OpenAlex / TeX / scholarly reconcile into graph truth
  (see ADR-055 candidate plane; S11 scholarly evidence remains candidate-only).
- Full remote GraphStore / multi-process materialization.
- DSPy / LLM extract (D136).
- Redesigning ontology node types (ADR-053 remains the article-shape ADR).
- **Non-article knowledge-base ontology** (textbook / chapter / concept / exercise
  node families, edges between chapters and papers, curriculum trees).
  That design is **explicitly later**, **before** treating non-article material
  as graph truth — see §2.11. This ADR only admits **loading** such sources into
  the catalog under the same plane.

---

## 2. Decision

### 2.1 Introduce the Universal Article Acquisition & Load Plane

A cross-cutting **load plane** sits **before and around** scientific content
processing:

```text
┌──────────────────────────────────────────────────────────────────┐
│ LOAD PLANE (this ADR)                                            │
│  locate → ensure catalog assets → select primary content         │
│  → invoke existing content pipeline stages → durable report      │
└──────────────────────────────────────────────────────────────────┘
         │ ensure assets only
         ▼
┌──────────────────────────────────────────────────────────────────┐
│ CONTENT PIPELINE (existing)                                      │
│  parse (GROBID) → structure → embed → materialize (HOT)          │
│  → extract → (later) enrich / resolve / summarize / tasks        │
│  ACAP evidence at each boundary (ADR-055)                        │
└──────────────────────────────────────────────────────────────────┘
```

**Binding rule:** anything that answers “how did this article enter the system
and what bytes were verified?” belongs to the load plane. Anything that answers
“what scientific structure/entities did we derive?” belongs to the content
pipeline. The two meet at **catalog-relative primary content paths + checksums**.

### 2.2 Single application entry: `ArticleLoadUseCase`

```text
ArticleLoadUseCase
  inputs:  LoadRequest { locator, policy, stages, eligibility, run_mode }
           # run_mode: inline | enqueue | worker  (see §2.5.1)
  deps:    SourceAcquisitionPort (+ registry),
           CatalogInventoryPort,          // existing
           CatalogMutationPort,           // existing (catalog writes only)
           IngestUseCase / batch_ingest,  // existing
           ExtractionUseCase (optional),  // existing
           PipelineCache (optional),      // existing — stage payloads only
           GraphScheduler surface (optional), // existing — enqueue/complete
           GraphStore (via ingest / scheduler),
           Clock / Id ports (determinism)
  output:  LoadReport (durable JSON) + optional .sgsnap
           # report includes cache hit/miss and queue task ids when used
```

Application **never** calls arXiv HTTP, `find`, or Python scripts directly.
CLI is a composition root only.

### 2.3 Provider-agnostic acquisition port

```text
SourceAcquisitionPort
  provider_code() -> source_code   // from data/source_codes.yaml
  can_resolve(raw_locator) -> bool
  resolve(raw_locator) -> ResolvedArticleRef
  ensure_assets(ref, AcquisitionPolicy) -> EnsuredAssets
```

**First implementable provider:** `arxiv`  
**Architecture must already admit the following families** (adapters may be
stubbed; **use-case control flow must not change** when they land):

| Family | `source_code` (DATA) | Locator examples | Typical primary | Notes |
|--------|----------------------|------------------|-----------------|-------|
| Preprint | `arxiv` | `2602.04837`, `arxiv:…`, `/abs|pdf|html/` | **PDF**; HTML/TeX sidecars | versionless id; category→catalog topic |
| Biomedical | `pubmed` / `europe_pmc` | `PMID:123`, `PMCID:PMC…`, pubmed.ncbi… URLs | **HTML/JATS** or PDF when licensed | PMC fulltext ≠ PubMed abstract-only |
| DOI-first metadata | `crossref`, `openalex`, `doi` | `10.…`, OpenAlex `W…` | **metadata only** by default | content fetch only via explicit policy + authorized landing |
| Scholar index | `semantic_scholar` | S2 paper id / DOI | metadata / abstract | not primary PDF scrape |
| CN academic | `cnki`, `wanfang`, `cqvip` (additive codes) | DOI/title/URL when legal | PDF/CAJ/HTML **policy-gated** | paywall/ToS; often local drop or licensed API |
| RU academic | `elibrary`, `cyberleninka`, `rsl` (additive) | eLIBRARY id, CyberLeninka URL, DOI | PDF/HTML **policy-gated** | same: no silent scrape |
| Curated HTML | `textbook`, `stanford` | catalog key / chapter URL | **HTML primary** | already in corpus; ParserPort path may differ |
| Local / operator | `local`, `user` | filesystem path, drag-drop | by extension (pdf/html/md) | reliability_tier 3 |
| Catalog pin | `catalog` (virtual) | `catalog:<source>/<topic>/<id>` | already-local only | offline; no network |

**Binding separations (ADR-043, `source_codes.yaml`):**

```text
source_code      WHERE it came from (arxiv | pubmed | cnki | elibrary | …)
source_type      MEDIUM of a given asset (pdf | html | markdown | latex | api_json)
source_profile   GENRE for parse/process (scientific_paper | textbook | …)
scientific_domain  WHAT field (cs.ml | medicine | …) — multi-valued, not a loader fork
```

OpenAlex / Crossref / Semantic Scholar remain primarily **enrichment or
identity resolution** ports, not silent fulltext scrapers. Fulltext acquisition
for paywalled CN/RU/publisher hosts is **license- and policy-explicit** (local
drop, institutional API, or user-provided bytes) — never “loader just downloads”.

**DATA registry:** new `source_codes` entries are added in
`data/source_codes.yaml` (and optional `data/acquisition_providers.yaml` for
locator patterns / default primary format) **before** adapters claim them.
Unknown `source_code` fails closed.

### 2.4 What “load” MUST include (anti-rework checklist)

The following are **in-scope for the plane’s contract**, even if some land in
later implementation waves. Omitting them from the ADR is how rework happens.

#### A. Identity & locator

1. **Normalized external identity** per provider (versionless arXiv id; stable
   catalog key; content digest for anonymous local PDF). A requested arXiv `vN`
   is a typed **source selector**, not part of Paper identity; it must survive
   resolve/acquisition/cache and must never silently fall back to latest.
2. **Raw locator preserved** on the report for audit (URL as typed, user input),
   together with the normalized id and optional requested revision.
3. **Provider discrimination** without stringly-typed `if arxiv` in application.
4. **Collision policy:** two locators resolving to the same canonical id are the
   same load target; conflicting checksums at the same id are **hard fail**
   (no silent overwrite) — aligns with `CatalogMutationPort` atomic-if-absent
   semantics for metadata and checksum-bound assets.

#### B. Catalog materialization

5. **Canonical catalog layout** under `data/article_catalog/article_catalog/`.
6. **Atomic writes** for metadata and assets (temp + rename; no torn files).
7. **Repo-relative paths** in durable reports when under the repo root.
8. **article.json** (or successor schema) updated/created with:
   - `schema_version`, `article_key`, `catalog_path`
   - `source_code`, `source_type` / profile from DATA not hardcode
   - identity block (title/authors when known)
   - `source_variants[]` with role/format/path/sha256/bytes
9. **Idempotent ensure:** second load with identical bytes →
   `Unchanged` / `catalog_hit`, not re-download and not re-hash failure.
10. **Indexes:** if atomic catalog indexes exist, load updates them through the
    same mutation surface (or explicitly defers with typed missingness) —
    no “PDF on disk but invisible to inventory”.

#### C. Multi-representation assets

11. **Primary content selection is DATA + profile driven**, not hardcoded to PDF:
    - `scientific_paper` + arXiv → **PDF primary**, HTML/TeX optional sidecars
    - `scientific_paper` + PubMed Central → **HTML/JATS primary** when fulltext
      available; PDF optional when present
    - `textbook` / lecture HTML → **HTML primary** (existing catalog path)
    - operator `local` → primary by file extension / declared MIME
    - metadata-only providers (Crossref/OpenAlex) → **no primary content** until
      a content-bearing ensure succeeds
    Selection is an explicit field on `EnsuredAssets.primary` (kind + path +
    sha256). Application never guesses “first file we found”.
12. **Optional representations** (HTML fulltext, JATS/XML, TeX source archive +
    inventory, Markdown, EPUB if ever needed) with **independent** checksums and
    status (`ok|skip|fail|not_requested|not_licensed`).
13. **Byte budgets / safety** for archives and HTML (TeX member limits, max HTML
    bytes, max PDF bytes) as policy constants on adapters — not infinite download.
14. **No representation is graph truth** until content pipeline stages run;
    sidecars feed candidate planes (tex inspect, scholarly reconcile, HTML
    structure extractors). Parser choice follows **primary kind**:
    GROBID for PDF; HTML/JATS parser port for HTML-primary; never force GROBID
    on pure HTML chapters.

#### D. Network & policy

15. **`AcquisitionPolicy` object** (not scattered CLI flags only):
    - `allow_network: bool` (default true; M462. false = CI/replay opt-out)
    - `include_tex_source: bool`
    - `include_html: bool` (default true for arXiv when network authorized)
    - `timeouts / retry / user_agent` (adapter config, not domain)
    - `rate_limit` hints (adapter)
16. **Offline catalog_hit path** must succeed with `allow_network=false`.
17. **Network failures are typed** (`rate_limited`, `not_found`, `forbidden`,
    `transport_error`) and appear on `LoadReport` — not only stderr.
18. **No credential leakage** in reports/logs (ADR-052).

#### E. Content pipeline handoff

19. **Resolve primary PDF path** (or future primary HTML path) for ingest.
20. **Call existing `IngestUseCase` / `batch_ingest_pdfs`** — do not fork a
    second materializer.
21. **Optional extract** via existing `ExtractionUseCase` as a **declared stage**,
    default on for single-article operator load, configurable off for bulk.
22. **Snapshot export** (`.sgsnap`) as a first-class stage with path + sha256.
23. **Process-local graph default** for single-article load; explicit policy if
    attaching to a long-lived store (never implicit shared DB in tests).

#### F. Assurance, eligibility, observability

24. **D127:** default locked; `--enable-import` / policy unlock is explicit,
    logged, and stamped on the report.
25. **ACAP source layer evidence:** load report fields must be mappable to
    `source_registered` / `source_verified` (checksum lineage, asset presence,
    typed missingness for optional assets).
26. **LoadReport schema** (`load-report.v1`) durable JSON:
    - locator raw + normalized
    - provider, source_code, category/topic
    - per-stage status + duration_ms
    - assets[]: kind, path, sha256, bytes, disposition
    - **cache[]:** stage_id, `hit|miss|bypass|retryable_failure`, key_sha256,
      payload_sha256 when hit (ADR-055 cache lineage)
    - **queue:** run_mode `inline|enqueue|worker`, tasks_opened/completed
      (task_type + vid), retry findings
    - ingest/extract summaries (counts, not full dumps)
    - snapshot path + sha256
    - `import_eligible`
    - errors[] / findings[]
27. **Tracing (ADR-052):** spans `article_load`, `ensure_assets`, `cache_lookup`,
    `ingest_pdf`, `extract`, `export_snapshot`, `scheduler_open` with
    `paper_id` / `provider` / `cache_status` fields.
28. **Governor:** may later gate on load-report presence/integrity and cache
    lineage; product emits truth first (same pattern as ACAP S07→T05).

#### G. Composition & extensibility

29. **Stage enum** on `LoadRequest` so pipeline DSL (ADR-049) can run
    `ensure_only | ensure+ingest | ensure+ingest+extract | full`.
30. **Run mode** on the same use case: `inline | enqueue | worker` (§2.5.1) —
    bulk is N× single load + graph queue, not a second loader.
31. **Batch load** shares policy and aggregate report; discovery only via
    acquisition/catalog ports (no permanent CLI `find`).
32. **Dry-run / plan-only** mode: resolve + describe assets + would-be cache keys
    without network and without graph writes (operator safety).
33. **Test seams:** all network and filesystem side effects behind ports;
    ADR-051 contract suite for `SourceAcquisitionPort` against mock + arXiv
    adapter (catalog_hit fixture mandatory; network tests optional/nightly);
    cache key stability tests; queue reopen idempotence tests.

### 2.5 Explicit stage model (aligned with ACAP)

| Load stage | ACAP relevance | Default single-article | Bulk default |
|------------|----------------|------------------------|--------------|
| `resolve` | source_registered | on | on |
| `ensure_assets` | source_verified | on | on |
| `ingest` (parse+structure+embed+materialize) | parsed…materialized | on | on |
| `extract` | extracted (+ evidence if produced) | on | off or separate wave |
| `export_snapshot` | supports checkpoint later | on | on |
| `emit_load_report` | assurance evidence | on | on |

**Not in default load** (separate use cases, may be chained later):

- OpenAlex enrich (`EnrichUseCase`)
- entity resolution / clusters / extractive summary verticals
- scholarly multi-provider reconcile (candidate)
- semantic graph task suite / ontology acceptance (S14)
- import unlock

Chaining is allowed via pipeline DSL or explicit CLI subcommands; it must not
be smuggled into “load” without stage flags.

### 2.5.1 Cache and queues — part of the one loader (not a second product)

The project **already** has the right primitives. ADR-056 does **not** invent a
new cache store or a new queue daemon. It binds them under `ArticleLoadUseCase`
so bulk load, retry, and stage replay do not become parallel architectures.

```text
                    ArticleLoadUseCase  (one loader)
                           │
           ┌───────────────┼───────────────────────────────┐
           ▼               ▼                               ▼
   SourceAcquisition   PipelineCache                  GraphScheduler
   (ensure assets)     (stage payloads)               (work / retry)
           │               │                               │
           ▼               ▼                               ▼
   catalog files      content-addressed              SchedulerTask nodes
   (bytes+json)       FilePipelineCache              in Samyama (ADR-040)
                      StageCacheKey
```

#### A. Two different “memories” (do not collapse them)

| Memory | What it stores | Identity | Who owns it | Graph mutation? |
|--------|----------------|----------|-------------|-----------------|
| **Catalog ensure** | primary/sidecar **source bytes** + `article.json` | provider id + asset role + **sha256** | `SourceAcquisition` + `CatalogMutationPort` | **never** |
| **Stage cache** | **derived stage payloads** (parse TEI, extract JSON, …) | `StageCacheKey` = `SourceIdentity` + stage + deps + impl/config/schema (+ prompt/model) | `PipelineCache` / `FilePipelineCache` | **never** (payload only) |
| **Work queue** | **what still needs to run** (ingest/extract/enrich/…) | `(task_type, article_id)` → `SchedulerTask` vid | `GraphScheduler` | **yes** — lifecycle nodes only (`import_eligible=false`) |
| **Graph truth** | scientific nodes/edges/vectors | ontology vids | content stages via `GraphStore` | **yes** — and **outside** stage-cache hits for mutation |

**Binding:** a cache hit is not a catalog ensure; a queue completion is not a
cache write; a graph write is never “cached” as truth.

#### B. Content-addressed stage cache (reuse existing port)

Use **`PipelineCache` + `StageCacheKey` + `SourceIdentity`** as already defined
in `da-ports` (M371 S01). Load plane rules:

1. **`SourceIdentity` is produced by acquisition**, not invented by parse:
   - `article_id` = normalized load identity
   - `selected_format` = primary content for this run (`Pdf` / `Html` / …)
   - `pdf_sha256` / `html_sha256` filled from **ensured catalog assets**
     (never one ambiguous “content hash” for both formats)
   - `acquisition_fingerprint` = policy+provider contract that produced those
     assets (e.g. ensure profile, not wall-clock)
   - `normalization_fingerprint` = locator/id normalization contract
2. **Cacheable load-related stages** (payloads only):
   - parse / structure serialization (GROBID TEI or normalized parse artifact)
   - extract artifact (when extract is a declared stage)
   - optional future summary/LLM stages (already model/prompt fields on key)
3. **Not cacheable as stage payloads** (always live observation or graph):
   - network ensure (but **catalog_hit** is free offline)
   - HOT graph materialization / any `GraphStore` mutation
   - snapshot export bytes may be content-addressed as **artifact**, but graph
     materialize must still obey ADR-055 replay rules when re-applied
4. **Hit policy (ADR-055):** exact key + exact payload checksum; retry/timeout
   knobs do **not** change the semantic key; changed impl/config/schema → miss.
5. **Failure cache:** `RetryableFailure` entries expire (`retry_after`); they
   never masquerade as success (existing `StageOutcome`).
6. **Loader wiring:** `ArticleLoadUseCase` optionally takes `Arc<dyn PipelineCache>`
   and builds keys from acquisition output. Missing cache port ⇒ always miss
   (single-article path stays simple).

#### C. Graph-backed work queue (reuse existing scheduler)

Use **`SchedulerTask` nodes + `TaskType` + `GraphScheduler`**, already aligned
with ADR-040 (no JSONL queue of record).

1. **Load does not replace the scheduler** — it **enqueues and/or runs** stages.
2. **Modes on the same use case** (policy, not separate products):
   - `inline` (default for `da load <one id>`): run selected stages now;
     optionally record `SchedulerTask` completion for observability
   - `enqueue`: resolve+ensure (optional), then open/update `SchedulerTask`
     rows for remaining stages; worker is existing `da scheduler-run` path
   - `worker`: process due tasks by calling the **same** stage functions as
     inline load (no second ingest implementation)
3. **TaskType mapping** (extend carefully; do not invent parallel enums):

   | Stage / work | `TaskType` (existing or additive) | Priority (ADR-037) |
   |--------------|-----------------------------------|--------------------|
   | ensure+ingest primary | `Ingest` | High |
   | re-parse after parser/profile change | `Reparse` | High |
   | extract after ingest | `Extract` | Medium |
   | OpenAlex metadata | `OpenAlexEnrich` | Medium |
   | re-embed | `EmbedStale` | Low |
   | deferred graph write batch (if any) | `GraphWrite` | Low |
   | acquire-only / rate-limited ensure | **additive** `Acquire` or fold into `Ingest` with finding — prefer **fold** until metrics demand a type | High when blocking ingest |

4. **Idempotent queue identity:** one node per `(task_type, article_id)` via
   `scheduler_task_vid` (already implemented). Re-load reopens completed tasks
   instead of duplicate pending rows.
5. **Rate limits / backoff:** network acquisition and OpenAlex failures use
   existing retry policy (`RetryPolicy`, `next_retry`); LoadReport records
   `rate_limited` findings; worker respects `next_retry`.
6. **Bulk load = N× single load + queue**, not a different architecture:
   `da load --ids-file … --mode enqueue` enqueues; workers drain.
7. **D127:** SchedulerTask nodes stay `import_eligible=false`; queue state is
   never scientific import authority.

#### D. What must appear on LoadReport for cache/queue honesty

- per stage: `cache=hit|miss|bypass|retryable_failure` + `key_sha256` (optional
  redact full key in operator logs; full key in durable report)
- acquisition: `catalog_hit|downloaded|unchanged|failed` + asset sha256s
- queue: `mode=inline|enqueue|worker`, `tasks_opened[]`, `tasks_completed[]`
- never claim ACAP `source_verified` from a cache hit alone without asset
  checksums present in catalog

#### E. Anti-patterns (rework if done)

- New Redis/SQLite/JSONL queue “for load only”
- Caching graph snapshots as substitute for `StageCacheKey` lineage
- Using OpenAlex enrich queue as the only bulk-ingest mechanism
- Silent re-download when catalog sha matches (must be `Unchanged`)
- Cache key that includes wall-clock, random temp paths, or port numbers
- Application calling `FilePipelineCache` paths directly (port only)

### 2.6 Catalog path contract (binding)

For source_code `arxiv`:

```text
data/article_catalog/article_catalog/arxiv/<fs_category>/<versionless_id>/
  article.json                  # bare/latest metadata
  article.html                  # bare/latest optional HTML
  article.vN.json               # exact requested revision provenance
  article.vN.html               # exact revision optional HTML
  source/
    <versionless_id>.pdf        # bare/latest primary
    <versionless_id>vN.pdf      # exact requested revision primary
    <source_key>.tar.gz         # optional TeX
    <source_key>.tex-inventory.json
```

- `fs_category` = arXiv category with `.` → `-` (existing convention).
- Catalog unit/Paper lookup is **versionless id**. Asset lookup is exact:
  accepting `vN` preserves it as `revision: N` and selects `<id>vN` assets.
- A versioned request must not reuse `<id>.pdf`, overwrite bare/latest assets,
  or silently fall back to latest. Checksums remain authoritative evidence.
- HTML/PDF discovery must not require the operator to pass filesystem paths
  when a locator is sufficient.

For curated HTML / textbook (existing layout; do not force arXiv shape):

```text
data/article_catalog/article_catalog/<source_code>/html/<unit_key>/
  article.json
  source/
    chapter.html                # primary for HTML units
```

Example today: `gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs/`.

- Other source_codes define their own layout under
  `article_catalog/<source_code>/…` without changing the use case.
- **`source_code` used on disk MUST be registered** in `data/source_codes.yaml`
  before load treats the unit as known (catalog-only codes like `gnn_textbook`
  that are missing from YAML are **registry debt** — fix DATA first, not load
  hardcodes). Container/work title (e.g. “GNN Textbook”) is metadata on
  `article.json` / future ontology, **not** a second loader product.

### 2.11 Non-article knowledge units (textbooks, chapters, lectures)

**Product fact:** the corpus is not only scientific articles. Curated HTML
(GNN textbook chapters, Stanford notes, …) is a real catalog family and a
known operator debt (partial ensure: few chapters vs full book TOC).

**Binding split of concerns:**

| Layer | When | What |
|-------|------|------|
| **Load plane (this ADR)** | **now** | resolve → ensure catalog assets → optional parse/ingest of **primary bytes** → LoadReport; same `ArticleLoadUseCase` / ports |
| **Non-article ontology** | **later, before graph truth** | node/edge types for Book/Chapter/Concept/Definition/Exercise/… and **links to Paper/Entity** |
| **Graph materialization of KB structure** | **after** ontology ADR | promote structure only under typed contracts + D127 |

**Loader rules for non-article units (binding now):**

1. Same plane, different **`source_profile`** / primary format — **not** a
   parallel `load-textbook` architecture.
2. Unit of load = **catalog unit** (one chapter / about page / lecture), not
   “the whole book as one blob” unless the provider defines a container ensure
   that still writes **per-unit** catalog rows.
3. Default stages for non-article: **`ensure` (+ optional `ingest` of HTML
   structure into process-local graph for debugging)** — **not** article-shaped
   extract/enrich/acceptance packages designed for papers.
4. LoadReport must stamp:
   - `source_code`, `source_profile` (e.g. `textbook`)
   - `content_kind` / primary (`html`)
   - `unit_key` (e.g. `gnn-ch-chapters__06-gnn-foundations`)
   - optional `container_ref` (stable id/title of the book — **string metadata
     only**, not an invented graph edge)
   - `graph_readiness: not_claimed` for non-article structure until a future
     ontology+materialize ADR claims it
5. **Do not invent article↔chapter edges in the load plane.** No silent
   `CITES` / `hasPart` / curriculum links from loader “helpfulness”. Those
   relationships require an ontology design that answers:
   - chapter → concepts/methods defined
   - paper → methods used / evaluated
   - shared Entity identity across article and textbook surfaces
   - evidence/provenance per link
6. Full-book acquisition (e.g. remaining GNN textbook chapters 03–26) is an
   **ensure/batch-load** problem under this plane + catalog registry — **not**
   blocked on the future ontology ADR, and **not** an excuse to sketch graph
   types in load code.

**Deferred (explicit future ADR, not this one):**

- Node families beyond Work-as-article for textbooks (Book, Chapter, …).
- Edge vocabulary paper↔chapter↔concept (and whether Entity is shared).
- Semantic graph tasks / acceptance for textbook KB.
- Claiming `graph_readiness` for non-article units.

Until that ADR is Accepted, non-article content is **catalog + report (+ optional
process-local parse)** only.

### 2.7 CLI surface (composition only)

```text
da load <LOCATOR>
  --provider auto|arxiv|local|catalog|textbook   # default auto via can_resolve
  --disallow-network                    # CI/replay; network is on by default
  --include-html / --include-tex-source
  --stages full|ensure|ingest|ingest+extract
  --skip-extract                        # deprecated alias of stages
  --output <snap.sgsnap>
  --report <load-report.json>
  --enable-import                       # D127 explicit
  --dry-run
```

Examples:

```text
da load 2602.04837 --report artifacts/loads/2602.04837.json
da load https://arxiv.org/html/2602.04837v1
da load catalog:arxiv/cs-ai/2602.04837 --stages ingest+extract
da load ./papers/x.pdf --provider local --output /tmp/x.sgsnap
# non-article unit: ensure (+ optional HTML ingest) — not ontology edges
da load catalog:gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs --stages ensure
da load https://anvithpothula.github.io/graph-neural-networks-textbook/chapters/06-gnn-foundations/ \
  --provider textbook --stages ensure
```

**Forbidden:** new top-level `da load-arxiv` as the long-term API.  
A temporary alias may exist during migration but must call `ArticleLoadUseCase`
with `provider=arxiv`.

### 2.8 Relationship to existing components

| Component | Relationship |
|-----------|--------------|
| `IngestUseCase` | **Reused** as content handoff; not rewritten |
| `batch_ingest_pdfs` | **Reused** for multi-id; discovery becomes acquisition-driven |
| `ExtractionUseCase` | Optional load stage |
| `CatalogInventoryPort` | Used for hit detection / inventory |
| `CatalogMutationPort` | Used for atomic catalog writes |
| `OpenAlexClient` | **Not** part of default load; enrich remains separate (queue: `OpenAlexEnrich`) |
| `ParserPort` / GROBID | Behind ingest only; parse **payload** may be stage-cached |
| `PipelineCache` / `StageCacheKey` / `SourceIdentity` | **Reused** for derived stage payloads; load **fills** `SourceIdentity` from acquisition |
| `PipelineStageExecutor` | Optional wiring for cacheable stages under same loader |
| `GraphScheduler` / `TaskType` / `SchedulerTask` | **Reused** graph work queue; load enqueues or runs inline — no second queue product |
| ~~`scripts/download_arxiv_to_catalog.py`~~ | ~~Temporary adapter backend~~ — **Replaced** by `da acquire-arxiv` (Rust) in M386 S04; archived to `archive/python-era-cold-s04/` (D237). Rust `ArxivSourceAcquisition` is now the validated sole acquisition port. |
| draft `crates/da-cli/src/load_arxiv.rs` | **Non-binding**; delete or reduce to thin alias after `da load` lands |

### 2.9 Failure model

| Failure | LoadReport | Graph | Catalog |
|---------|------------|-------|---------|
| Invalid locator | Reject, no stages | untouched | untouched |
| Network denied & missing PDF | ensure failed | untouched | untouched |
| Network authorized, 404 | ensure failed typed | untouched | no partial primary |
| PDF ok, HTML fail | ensure partial; finding on HTML | ingest may proceed if primary ok | PDF kept; HTML status fail |
| Ingest fail after ensure | ensure ok, ingest fail | process-local may be dirty → **do not export success snapshot**; report error | assets remain (content acquisition succeeded) |
| Extract fail after ingest | ingest ok, extract fail | snapshot optional policy: export ingest-only snap with finding | unchanged |
| Checksum conflict on ensure | hard fail | untouched | no overwrite |

**Binding:** success snapshot implies ingest stage ok.  
Partial success is allowed only with `ok=false` or staged ok flags — never a
green exit that hides failed ensure/ingest.

### 2.10 Lifecycle tags (ADR-039)

| Element | Initial tag | Gate to next |
|---------|-------------|--------------|
| Domain locator/report types | `[bounded]` after unit tests | — |
| `SourceAcquisitionPort` + mock | `[bounded]` after contract tests | — |
| `ArticleLoadUseCase` offline | `[bounded]` after app tests | — |
| ArXiv catalog_hit path | `[bounded]` on real catalog ids | `[validated]` after N-id batch metrics |
| ArXiv network fetch (Rust or script-behind-port) | `[proposed]`→`[bounded]` with fail-closed tests | `[validated]` after rate-limit + checksum replay |
| `da load` CLI | `[bounded]` when single command works for catalog_hit + network | docs + governor optional |
| Non-arXiv providers | `[proposed]` stubs ok | separate slices |

---

## 3. Architecture (onion)

```text
da-cli          composition: parse LoadRequest, wire adapters, print report
da-application  ArticleLoadUseCase, stage orchestration, D127 policy apply,
                SourceIdentity construction, cache lookup/store, queue open/complete
da-ports        SourceAcquisitionPort, AcquisitionPolicy, EnsuredAssets DTOs
                + existing Catalog*, Parser, Embedder, GraphStore, Extractor
                + existing PipelineCache, PipelineStageExecutor
da-adapters     ArxivSourceAcquisition, LocalPathAcquisition, Catalog FS
                FilePipelineCache (existing)
                (Python script optional subprocess only behind acquisition adapter)
da-domain       ArticleLocator, ResolvedArticleRef, LoadReport, stage enums
                + existing TaskType / PendingTask (queue identity; no new queue model)
kg-*            untouched by load plane (no acquisition types in kg-ontology)
```

**Invariant:** `da-application` has **zero** `reqwest` / `std::process::Command`
to downloaders.  
**Invariant:** `da-domain` has **zero** filesystem paths as required types
beyond optional stringly report fields produced by application.  
**Invariant:** stage cache never stores graph mutations; queue nodes never imply
scientific import authority (`import_eligible=false`).  
**Invariant:** bulk path uses the same use case as single-id (`inline` vs
`enqueue`/`worker`), not a second loader.

---

## 4. Consequences

### Positive

- One operator entry; second provider does not fork CLI architecture.
- Offline reproducibility (catalog_hit) and online recovery share one report.
- ACAP source layer can consume real ensure evidence.
- D127 and network authorization remain explicit.
- Aligns with catalog atomic mutation and multi-rep article.json already used
  by textbook HTML corpus.
- **Reuses** existing `PipelineCache` / `SourceIdentity` and `GraphScheduler`
  so bulk, retry, and stage replay do not invent Redis/JSONL/second-queue.
- Cache hits stay content-addressed (ADR-055); queue is graph lifecycle only
  (ADR-040).

### Negative / cost

- Up-front port surface larger than “wrap the python script in clap”.
- Temporary dual implementation (script behind adapter) until Rust fetch is
  validated.
- Batch-ingest discovery refactors to avoid permanent `find` in CLI.
- Wiring cache+queue into the loader needs careful stage boundaries so graph
  materialize is never “cached as truth”.

### Risks if we skip this ADR

- `da load-arxiv` becomes permanent product API → textbook/DOI paths clone it.
- Checksums and catalog writes stay inconsistent between script and ingest.
- ACAP `source_verified` remains synthetic while “load” means “hope PDF exists”.
- Network policy drifts (some paths fetch silently).
- Bulk “queue” becomes a second product (JSONL/Redis) disconnected from
  `SchedulerTask` and stage cache keys.
- Operators re-download forever because ensure and stage-cache are confused.

---

## 5. Implementation waves (after Accept)

| Wave | Deliverable | Proof |
|------|-------------|-------|
| **W0** | This ADR Accepted; reject provider-rooted CLI as design | review |
| **W1** | Domain: locator, policy, LoadReport schema (+ cache/queue fields) + pure tests | `da-domain` tests |
| **W2** | Ports: `SourceAcquisitionPort` + mock contract suite | ADR-051 port contour |
| **W3** | `ArticleLoadUseCase` offline: mock acquisition + ingest seams; optional `PipelineCache` miss path | app tests |
| **W4** | `ArxivSourceAcquisition` catalog_hit + optional script ensure; **build `SourceIdentity` from assets** | `2602.04837` offline green |
| **W5** | CLI `da load` + durable report; delete/alias `load_arxiv` draft | operator demo |
| **W6** | Wire parse/extract stages through existing `PipelineCache` keys (hit/miss on report) | cache contract tests |
| **W7** | Batch load via acquisition; remove CLI `find` discovery | multi-id report |
| **W8** | `run_mode=enqueue|worker` on same use case; reuse `GraphScheduler` / `TaskType` | dual-run idempotent queue |
| **W9** | LocalPath + Catalog providers | fixtures |
| **W10** | Rust-native arXiv fetch behind same port; script demoted | network tests nightly |
| **W11** | ACAP mapping from LoadReport → source layer (+ cache lineage fields) | assurance tests |
| **W12** | Optional pipeline DSL nodes for load stages | ADR-049 alignment |

Waves **W1–W5** are the minimum for “one loader works offline/online”.  
**W6–W8** are the minimum so bulk/retry/cache do not spawn a second architecture.  
W9–W12 prevent the next generation of hacks.

---

## 6. Testing requirements (ADR-051)

| Contour | Required |
|---------|----------|
| Domain unit | locator normalize (URL/html/pdf/version), policy defaults, report digest |
| Port contract | mock acquisition: resolve/ensure dispositions; fail-closed network |
| Application | load with catalog_hit; ingest failure leaves report honest; D127 stamp |
| Adapter | arXiv catalog fixture; atomic catalog write; checksum conflict |
| E2E (optional merge) | `da load 2602.04837` offline after catalog seed |
| Architecture | no application→adapters inversion; no kg-* acquisition types |

---

## 7. Acceptance criteria for “load is done” (product)

1. Operator runs **one** command with a locator (not a hand-built absolute PDF
   path) for an arXiv id already in catalog → ingest snapshot + load report.
2. Same command on a missing id → catalog materialize + ingest
   (or typed failure, never hang without report). `--disallow-network`
   keeps the miss typed NetworkDenied for CI/replay.
3. Second identical run → ensure `Unchanged` / catalog_hit; no checksum fight.
4. Report always has `import_eligible=false` unless explicit unlock.
5. No `da-application` dependency on arXiv URLs or download scripts.
6. Adding `local` provider does not change `ArticleLoadUseCase` control flow.

---

## 8. Supersession / amendments

- Does **not** supersede ADR-055; **feeds** its `source_*` states with real
  evidence.
- Does **not** supersede ADR-053; load stops at verified source + content
  handoff; layered summaries/entities remain preparation verticals.
- Amends operator practice implied by ADR-039 ingest lifecycle: Stage 0/1
  acquisition is no longer “Python M061 only” — it becomes a first-class Rust
  plane with the same fail-closed rules.

---

## 9. Open points (explicitly deferred, not forgotten)

1. Whether HTML-primary ingest (textbook) uses a different `ParserPort` path
   inside the same load use case (likely yes; still acquisition-selected
   primary content). **Default for non-article: ensure-only until ontology ADR.**
2. DOI → PDF legal/source policy (publisher APIs vs arXiv dual-home).
3. Governor rule consuming `load-report.v1` (after product fixtures exist).
4. Exact `article.json` schema version bump for universal `source_variants`
   if current arXiv drops lack it — additive fields only.
5. ~~**Registry debt**~~ — closed in M372: `gnn_textbook` / `company_blog` /
   `nature` / `local` registered in `data/source_codes.yaml`.
6. ~~**GNN textbook completeness**~~ — closed: TOC ensure 28/28 via
   `da load --batch-toc data/source_catalogs/gnn_textbook_toc.yaml`.
7. **Non-article KB ontology** — **ADR-057 Proposed** (Collection /
   KnowledgeUnit + paper↔chapter link model). Must be Accepted before graph
   promotion of textbook structure; load stamps `container_ref` /
   `source_profile` metadata only (`graph_readiness=not_claimed`).

These must not block W1–W5; they must not be “solved” by hardcoding arXiv-only
CLI or by inventing textbook ontology inside the loader.

---

## 10. Status action

**Accepted** 2026-08-10 (architecture summary confirmed by user).

Post-accept checklist:

1. ~~Update `doc/adr/ADR-INDEX.md`.~~
2. ~~Record decision in GSD DECISIONS (D###) pointing at this ADR.~~
3. Open implementation milestone/slice mapped to waves **W1–W5** (domain → ports → offline use case → arXiv ensure → `da load`).
4. Mark any arXiv-only CLI draft (`load_arxiv.rs`) as non-goals / delete in W5.
5. Non-article ontology + paper↔chapter links remain a **separate future ADR**
   before graph promotion of textbook structure; full GNN textbook ensure is
   load-plane work under §2.11, not ontology.
