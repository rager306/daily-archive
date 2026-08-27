# M463 — GROBID TEI lift matrix versus ParsedArticle

Milestone: M463-snmgcd / S01 / T03. Sources:
`artifacts/research/m463/tei-inventory-2608.22118.json`,
`artifacts/research/m463/tei-inventory-2507.19457.json`.

Both fixtures parsed with the structural profile. Counts are raw TEI element
totals versus what `ParsedArticle` carries today. This matrix ranks tissue for
a later lift; it performs no code lift itself. D127 stays locked: TEI here is
sidecar evidence, not graph truth.

## Legend

- TEI = count inside GROBID TEI XML.
- PA = count present as a `ParsedArticle` field after the current lift
  (`crates/da-parse` projector, ex-adapters `grobid_parser.rs`).
- Gap = TEI present and PA absent or lossy.

## Matrix

| TEI family | Paper | TEI | PA | Gap | Rank |
|---|---|---|---|---|---|
| header author / persName | 2608.22118 | 1 | 0 (`has_authors_field=false`) | yes | **P0** |
| header author / persName | 2507.19457 | 21 | 0 | yes | **P0** |
| article idno arXiv (revision) | both | 1 each (`arXiv:…vN[cs.CL]`) | not lifted; ADR-065 re-scans tei_xml string at materialize | yes | **P0** |
| non-table figures (figure without type=table) | 2608.22118 | 48 | 0 (`has_figures_field=false`) | yes | **P0** |
| non-table figures | 2507.19457 | 22 | 0 | yes | **P0** |
| table figures | 2608.22118 | 4 | 4 (`tables=4`) | no | kept |
| table figures | 2507.19457 | 3 | 3 | no | kept |
| formulas | 2608.22118 | 0 | 0 | n/a | kept |
| formulas | 2507.19457 | 24 | 24 (real-TEI test locks extractor) | no | kept |
| sections / head | 2608.22118 | head 37, div-path sections 83 | 83 sections, all `path_id` | parity ok | kept |
| sections / head | 2507.19457 | head 20 | extracted (bibliography + sections tests) | parity ok | kept |
| biblStruct citations | 2608.22118 | 15 | 14 citations (6 DOI, 2 arXiv, 8 title) | minor | kept |
| biblStruct citations | 2507.19457 | 81 | extracted (123 ref bibr anchors) | minor | kept |
| in-text refs xref figure/table | 2608.22118 | 18 fig + 2 tab | anchors 18 cited, none bound to figure index | binding gap | **P1** |
| in-text refs xref figure/table | 2507.19457 | 24 fig + 5 tab | same shape | binding gap | **P1** |
| ptr / url (telemetry links) | 2608.22118 | 14 | none | yes | **P1** |
| ptr / url | 2507.19457 | 66 | none | yes | **P1** |
| citation authors (persName in biblStruct) | 2608.22118 | 68 persName total | citation objects carry title/DOI only | yes | **P1** |
| note (footnote/margin) | 2608.22118 | 2 | none | yes | **P2** |
| note | 2507.19457 | 116 | none | yes | **P2** |
| graphic elements outside figure | 2608.22118 | 28 | 0 | folds into P0 figure lift | P0-adjacent |
| affiliation | both | 0 | n/a | none observed | skip-for-now |
| funder | both | 0 | n/a | none observed | skip-for-now |
| keywords | both | 0 | n/a | none observed | skip-for-now |
| idno DOI | 2608.22118 | 6 | 6 DOIs live inside citations | covered by citations | kept |

## Ranked action list

### P0 (structural identity lost today)

1. **arXiv idno revision** — paper version identity (`arXiv:2608.22118v1`) never
   reaches `ParsedArticle`; materialize re-parses the TEI string instead. Lift
   once, bind revision at parse time.
2. **Header authors** — 1 author on the structural fixture, 21 on the gold
   fixture, all dropped. `has_authors_field=false` confirmed on both.
3. **Non-table figures** — 48 and 22 respectively, zero indexed. TeiFigure
   machinery exists off-article (`da-domain::tei_figures`) but the pipeline
   article does not carry them yet.

### P1 (linkage lost)

4. In-text figure/table refs (`ref[@type=figure|table]`) — present (18+2, 24+5)
   but unusable because no figure/table node exists to bind to; unlocks after
   P0 item 3.
5. `ptr`/`url` telemetry links — 14 and 66 occurrences dropped entirely.
6. Citation authors — persName lists inside biblStruct are discarded; only
   DOI/arXiv/title survive on citation objects.

### P2 (deferred enrichment)

7. Notes (footnotes/margin notes) — 116 on the gold fixture; structured field
   proposed in earlier wave design, no PA surface today.
8. Formulas materialization — extraction works (24/24 on gold), downstream
   rendering surface is thin.
9. Table-cell extraction surface — tables carried whole; cell-level lift stays
   out until a consumer exists (ADR-071 partial precedent).

### Skip (explicitly out)

- Nested div rewrite — current flat section model passes parity checks;
  a hierarchy rewrite adds risk without a reader today.
- DeLFT — out of scope by research decision; heavy Java ML stack.
- Patent/training APIs — wont-list item of the REST subset (S02).
- MinerU/Docling/Marker — MD backends, not TEI clones; routing candidates only,
  never treated as TEI replacements.
- TEI-as-graph-truth — forbidden by D127 regardless of lift depth. Lift
  enriches the sidecar plane only.

## Consequences for later slices

- P0 trio matches the roadmap headline numbers (48 vs 0 lifted; author present
  in TEI, absent in PA; idno v1 in TEI).
- Any future lift unit lives in `da-parse` IR + projector along KTD1 seams;
  the this-cut IR deliberately omits all three P0 tissues, so lifting requires
  an explicit IR change, not a projector patch.
- REST subset freeze (S02) needs the processFulltextDocument contract plus
  isalive; this matrix confirms nothing else in daily use depends on other
  endpoints.
