# ADR-077: Independent PDF/layout Input Plane — Existing Rust Donor, Java-free Slice Boundary

- Status: Accepted
- Date: 2026-09-30
- Deciders: repository owner + agent (D498/D499 recorded by agent under autonomous auto-mode)
- Decisions: D498 (low-level PDF/layout engine and Java-free slice boundary), D499 (Java-free measurement is scoped/diagnostic); inherits D127, D321, D377, D491, D492
- Requirements: R140 (primary); relates R123, R124, R131, R133, R137, R141
- Milestone: M514-1lss2s
- Amends: none (ADR-076 owns cascade sequencing/honesty and is unchanged; ADR-075 owns the platform and is unchanged)
- Binding level: binding for the PDF/layout input engine choice and for the declared Java-free slice boundary; it does not amend ADR-076 waves, promotion rules, or the three-automata separation
- Revisable: after S04/S05 shows the token-coverage gap is real body-text loss inside P1, or that visual structure cannot be reconstructed natively; a second PDF engine then needs a new recorded decision, not a default
- Tags: pdf, layout, pdf-inspector, pdfalto, license, java-free, scoped-diagnostic, locks

## 0. One-line decision

> We will consume the PDF → layout → segmentation input through the **existing**
> Rust donor `pdf-inspector` (`PdfInspectorLayoutDonor`) plus
> `da_parse::segment_document_full`, and we will declare the Java-free vertical
> slice boundary as **P1 (layout lattice) and P2 (segmentation) only**.
>
> We will **not** create a second PDF engine, put `pdfalto` on the product path,
> move the product `java_independence` axis, or read the scoped diagnostic zero
> as product evidence.

## LLM Reading Notes

- ADR-076 owns cascade **sequencing and honesty** (three automata, waves W0–W6,
  graph freeze). This ADR owns **which low-level PDF engine the input plane
  uses** and **where the Java-free slice ends**. It does not touch waves.
- The engine is already in the tree. `PdfInspectorLayoutDonor`
  (`crates/da-adapters/src/layout_donors.rs:913`) calls pure Rust
  `pdf_inspector::extract_text_with_positions_mem`; `XyCutLayoutDonor`
  (`:354`) reorders; `da_parse::segment_document_full`
  (`crates/da-parse/src/native_segmentation.rs:842`) segments without subprocess
  or HTTP. Nothing here is new code.
- No second engine: D498, and the hard sketch forbids it before measuring the
  existing one. Measurement lives in `doc/review/grobid-absorption-s03-spike.md`.
- `pdfalto` is GPL-2.0 `blocked` and subprocess-only; it stays a recorded oracle
  lane, never the product route. Its binary is not installed locally.
- Two named, unfixed provenance gaps: `lopdf 0.41.0` (transitive to
  `pdf-inspector`) is in no license inventory; `pdf_inspector` is pinned as
  crates.io `0.1.7` but its license evidence points at the vendored `1.17.0`
  tree, and there is no `[patch]` section. MIT holds for both versions, but the
  evidence is bound to a tree that is not the one that compiles.
- The Java-free claim is **scoped and diagnostic**: P1/P2 only, three pinned
  documents, `claim_scope: diagnostic_only`. It does not move the product axis.
- Locks are unchanged: `deployed=false`, `import_eligible=false`,
  `auto_promote=forbidden`, `production_route=forbidden`,
  `graph_write=forbidden`; D127/D321 stay closed; no production splice.
- Read the slice declaration as DATA (`java_free_vertical_slice` block in
  `data/parse/layout_bakeoff.yaml`), and the receipt as
  `artifacts/parse_compat/java_free_vertical_slice.json`. The receipt is
  source-bound, not a self-declaration.

## 1. Context

ADR-075 shipped the staged native platform and kept Java GROBID as oracle and
rollback. ADR-076 fixed cascade sequencing, the three automata, and the graph
freeze, and left the PDF input itself untouched: its P1 stage is still "pdfalto /
layout lattice" with `production_authority = Java`. Nothing recorded which
low-level PDF engine the Java-free input plane may use, nor how far a Java-free
slice extends.

M514-1lss2s opens the absorption route from PDF without prior Java TEI. The hard
sketch requires a spike plus an ADR **before** any engine decision, and forbids
building a second PDF engine before the existing code is measured. The spike
(`doc/review/grobid-absorption-s03-spike.md`, S03/T01) measured the existing
route on the declared S13 corpus (3 documents / 202 pages) and answered the
engine question directly.

Measured state the spike recorded:

| Lane | 202-page token count | Coordinate coverage | Reproducible locally |
|---|---|---|---|
| `pdf_inspector_positioned` | 27878 | `complete` (`coordinate_valid == token_count`) | yes, retokened identical |
| `pdf_inspector_positioned_xy_cut` | 27878 | `complete` | yes, retokened identical |
| `pdfalto` | 91761 (recorded only) | `complete` (recorded only) | **no** — binary absent, GPL-2.0 blocked |

The spike's direct answer is "no second engine needed for P1/P2", with named
forward work: the ×3.29 token-coverage gap (unresolved granularity vs real text
loss), visual heads, and license/provenance. Both committed carriers disagree on
the `pdfalto` lane (`ok/40501` vs `terminal_failure/malformed`); the spike
classified that as a provenance finding, explained by differing binary
availability, and did not repair it.

What is missing without this decision:

- no recorded engine choice for the input plane (so "add a second engine" stays
  a default rather than an evidence-backed decision);
- no recorded boundary between the Java-free slice and the Java cascade;
- no recorded statement that the Java-free zero is scoped, so it cannot be
  misread as a product claim.

## 2. Decision

### We will

1. **Use the existing Rust donor as the input engine.** `pdf-inspector`
   (crates.io `0.1.7`, MIT, `license_status: verified` in
   `data/parse/license_inventory.yaml`) resolves as the positioned-token donor
   through `PdfInspectorLayoutDonor` (`crates/da-adapters/src/layout_donors.rs:913`),
   with `XyCutLayoutDonor` (`:354`) for reading order and
   `da_parse::segment_document_full` (`crates/da-parse/src/native_segmentation.rs:842`)
   for segmentation. **No second PDF engine is created** (D498).
2. **Keep `pdfalto` off the product path.** It stays the external
   GPL-2.0 `blocked`, subprocess-only oracle lane
   (`data/parse/license_inventory.yaml`), usable as recorded cross-lane evidence
   in shadow and never as the production route.
3. **Name the two provenance gaps instead of hiding them.** `lopdf 0.41.0`
   (transitive to `pdf-inspector`) is recorded in no license inventory; the
   obligation to add it belongs to whoever next touches that dependency. The
   `pdf_inspector` resolved build `0.1.7` diverges from the vendored `1.17.0`
   license-evidence tree with no `[patch]` section; MIT covers both versions, so
   this is a provenance binding defect, not a licensing defect. Neither is fixed
   here.
4. **Bound the Java-free slice explicitly.** P1 (layout lattice) and P2
   (segmentation) are inside the slice. P3..P9 are outside it with a typed
   reason, never a silent zero — see §3.
5. **Keep the Java-free claim scoped and diagnostic.** The slice receipt carries
   `claim_scope: diagnostic_only`; the product axis `java_independence` stays
   `not_measured` (`java_production_route_active`). A three-document slice is not
   a product-coverage denominator, and `data/parse/absorption_progress.schema.yaml`
   plus `absorption_progress_baseline.json` are not edited (D499).
6. **Keep every lock closed.** `deployed=false`, `import_eligible=false`,
   `auto_promote=forbidden`, `production_route=forbidden`,
   `graph_write=forbidden`; D127 and D321 stay closed; no production splice and
   no replacement of a Java stage by this slice.
7. **Bind the decision to measured evidence.** The engine choice is bound to
   `doc/review/grobid-absorption-s03-spike.md`, and the slice boundary is bound
   to the `java_free_vertical_slice` declaration block in
   `data/parse/layout_bakeoff.yaml` (additive, D498/D377) — not to a second
   contract file.

### We will not

- Create or adopt a second PDF engine for the input plane (D498).
- Add MinerU / Docling / pdffigures2 / Python OCR / a forked `pdfalto` as a
  product sidecar.
- Route the product parse path through the Java-free slice, or count the
  scoped zero as delivered product capability.
- Edit `data/parse/grobid_compatibility.yaml` or
  `data/parse/acceptance_manifest.yaml` (S02 byte pins), or edit
  `data/parse/absorption_progress.schema.yaml` /
  `absorption_progress_baseline.json` (frozen S02 hash chain).
- Rewrite ADR-076 or its census literals; this ADR is additive.
- Read the ×3.29 token-coverage gap as settled; it is open forward work.

### This decision authorizes

S03/T03..T06: the additive slice declaration, the typed Java-invocation observer
on the existing `ParseBackend::fetch_tei` seam, the source-bound receipt, the
negative controls, and the governor `java_free_slice_*` labels.

### This decision does not authorize

A production native input route, graph materialization, import, promotion of any
cascade family, or a change to the product `java_independence` axis.

## 3. Java-free slice boundary (P1/P2 in, P3..P9 out)

Stage IDs are read by path + pointer from
`data/parse/grobid_compatibility.yaml#defaults.cascade_stages`; they are not
duplicated as a second list.

| Stage | Name | In slice | Disposition on the slice |
|---|---|---|---|
| P1 | layout lattice | **yes** | `Observed(0)` under an installed guard — no Java attempt |
| P2 | segmentation | **yes** | `Observed(0)` under an installed guard — no Java attempt |
| P3 | header heads | no | typed `out_of_slice` (Java authority) |
| P4 | bibliography | no | typed `out_of_slice` (Java authority) |
| P5 | fulltext / sections | no | typed `out_of_slice` (Java authority) |
| P6 | figure / table / formula | no | typed `out_of_slice` (visual heads not measured) |
| P7 | coordinates / callouts | no | typed `out_of_slice` (Java `teiCoordinates`) |
| P8 | consolidation | no | typed `out_of_slice` (profile-dependent, Crossref) |
| P9 | envelope, lift, conservation | no | typed `out_of_slice` (Java TEI + `TeiLifter`) |

A zero on a slice stage must be `Observed(0)` with guard provenance; absence of
measurement is `NotObserved`, never `0`. A non-slice stage must carry its typed
`out_of_slice` reason and must not carry a numeric count.

## 4. License and provenance

| Component | Status | Evidence | Boundary |
|---|---|---|---|
| `pdf_inspector` | MIT, `verified` | `data/parse/license_inventory.yaml` (`pdf_inspector` entry) | cargo dependency |
| `pdf_inspector` resolved version | `0.1.7` (crates.io) | `Cargo.lock` (`name = "pdf-inspector"`) | compiled |
| vendored `pdf-inspector` | `1.17.0` | `/root/vendor-source/pdf-inspector/Cargo.toml` | **not consumed** — no `[patch]` |
| `pdfalto_runtime` | GPL-2.0, `blocked` | `data/parse/license_inventory.yaml` (`pdfalto_runtime` entry) | subprocess, not on product path |
| `lopdf` | **absent from every inventory** | `Cargo.lock` (`lopdf 0.41.0`) | transitive PDF byte reader |

Named gaps, deliberately not closed here:

1. **License-evidence version divergence.** The inventory's `license_evidence`
   points at the vendored `1.17.0` tree while `0.1.7` is what compiles and no
   `[patch]` consumes the vendored tree. MIT covers both, so this is a binding
   defect; a follow-up must either re-bind the evidence to the resolved version
   or add `[patch]` and re-measure.
2. **`lopdf` gap.** The library that physically reads PDF bytes is recorded in
   no `data/parse/*.yaml`. Adding it to the inventory is the obligation of
   whoever next touches that dependency (spike §5).
3. **`pdfalto` lane not executable.** GPL-2.0 `blocked`, no local binary, and
   the two committed carriers disagree on its outcome. Its numbers are recorded
   evidence only.

## 5. Locks unchanged

| Lock | Value | Reference |
|---|---|---|
| `deployed` | `false` | D127 / ADR-076 §14 locks |
| `import_eligible` | `false` | D127 / ADR-076 §14 |
| `auto_promote` | `forbidden` | D127 / ADR-076 §14 |
| `production_route` | `forbidden` | ADR-076 / D321 |
| `graph_write` | `forbidden` | ADR-076 / D321 |
| D127 / D321 | closed | no import, no graph, no production splice |

This ADR does not open any of them, and the slice it authorizes does not either.

## 6. Evidence binding

| Claim | Evidence |
|---|---|
| Engine choice for the input plane | `doc/review/grobid-absorption-s03-spike.md` §2, §7 (measured, reproducible) |
| Existing Rust route is Java-free | spike §2.1 (`Command::new` only inside `PdfaltoLayoutDonor`; `native_segmentation.rs` has no subprocess/HTTP) |
| Slice boundary is DATA | `java_free_vertical_slice` block in `data/parse/layout_bakeoff.yaml` (additive, D498/D377) |
| Stage IDs come from existing DATA | `data/parse/grobid_compatibility.yaml#defaults.cascade_stages` (read, not edited) |
| Corpus identity | pinned `sha256` per `source_pdf` in `data/parse/layout_bakeoff.yaml` |
| Product axis unaffected | `data/parse/absorption_progress_baseline.json` (`java_independence = not_measured`), D499 |
| Receipt | `artifacts/parse_compat/java_free_vertical_slice.json` (source-bound, redacted) |

## 7. Consequences

- The input plane has a recorded engine and a recorded slice boundary, so
  "add a second engine" is now a decision to be justified, not a default.
- The Java-free progress becomes visible as a source-bound scoped receipt plus
  governor `java_free_slice_*` labels inside the existing `absorption_progress`
  dimension — without pretending the product axis moved.
- Two provenance gaps stay visible as named, typed follow-ups rather than as
  silent debt.
- Downstream S04/S05 inherit the ×3.29 token-coverage gap as open work with an
  independent oracle on body characters, and visual structure stays out of this
  slice.
- If S04/S05 proves real body-text loss inside P1, the engine decision reopens
  (D498 trigger) and requires a new recorded decision.

## 8. References

- `doc/adr/ADR-075-scholarly-parse-plane-staged-platform.md` — platform; unchanged.
- `doc/adr/ADR-076-grobid-cascade-three-automata-graph-freeze.md` — cascade
  sequencing, P0–P9 stages, locks; unchanged.
- `doc/review/grobid-absorption-s03-spike.md` — measured spike that grounds this ADR.
- `data/parse/layout_bakeoff.yaml` — sanctioned corpus and slice-declaration carrier.
- `data/parse/license_inventory.yaml` — `pdf_inspector` (MIT) and `pdfalto_runtime` (GPL-2.0 blocked).
- `data/parse/grobid_compatibility.yaml` — `defaults.cascade_stages` (stage-ID source).
- `crates/da-adapters/src/layout_donors.rs`, `crates/da-parse/src/native_segmentation.rs` — the existing route.
