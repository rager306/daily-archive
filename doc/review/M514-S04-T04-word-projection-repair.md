# M514-1lss2s / S04 / T04 — native word projection repair

Status: **repair implemented and measured; T04 itself is not executed, not receipted,
and not completed.** This note records what was repaired, the exact evidence, and the
one residual blocker this repair does **not** own. Recovery for T04 stays a
supervisor decision.

Head at measurement: `b847386e669a4bc1c3f1d39917a3c2e8285180a3` (branch `feat/da-parse`).

## 1. What was broken (measured, not inferred)

The declared T04 pipeline

```
PdfInspectorLayoutDonor -> segment_document_full -> build_native_region_carrier -> T02/T03 bands
```

could not execute on **any** of the three pinned PDFs. All six band invocations failed
with the T02/T03 declared refusal `TokenContainsWhitespace`
(`gsd_exec[4fad8562-e4b6-4f4d-9102-26e80cff46a9]`, exit 0, 38 904 ms):

| document | header band | bibliography band |
|---|---|---|
| hub-2507.19457 | `{page 0, source 0}` | `{page 70, source 93}` |
| rlm-2512.24601 | `{page 0, source 0}` | `{page 9, source 8}` |
| reca-2605.26525v1 | `{page 0, source 0}` | `{page 9, source 52}` |

Root cause (WP-01): the positioned donor's lexical unit is the PDF **show-operator
span**, not a word. `crates/da-adapters/src/layout_donors.rs:857-876` copies
`pdf_inspector::extract_text_with_positions_mem`'s `TextItem.text` verbatim into
`LexemeRow`, and one `Tj`/`TJ` string may carry a whole line — so `LexemeRow.text`
legitimately contains internal whitespace while the bands contract on word
granularity. Guards: `crates/da-parse/src/native_header_native_input.rs:400`,
`crates/da-parse/src/native_bibliography_native_input.rs:748`.

Second, independent defect (WP-02): even at word granularity, the S03 region walk
could not produce honest regions. It promotes a region when a **single token** equals
a marker word (`crates/da-parse/src/native_segmentation.rs`, `full_marker`) over a
lattice that collapses each page into one block and one line
(`layout_donors.rs:405`), so it is simultaneously too strict and too loose. Measured
S03 census (`gsd_exec[60cbe5c9-b0ec-4025-bddb-728622c77932]`):

| document | S03 regions |
|---|---|
| hub-2507.19457 | header 10 759, references 3 359, **no body** |
| rlm-2512.24601 | header 40, body 4 681, references 3 609 |
| reca-2605.26525v1 | header 1 972, references 3 458, **no body** |

The S04/T03 oracle had already recorded the inverse failure as a fixture limitation
(`crates/da-parse/tests/native_bibliography_native_input_oracle.rs:38-41`): a standalone
marker word anywhere flips the region, so a reference containing `Methods` turns the
rest of the document into Body.

## 2. What this repair adds (additive only)

| module | role |
|---|---|
| `crates/da-parse/src/native_word_projection.rs` | deterministic partition of a show-operator span into word tokens |
| `crates/da-parse/src/native_heading_regions.rs` | geometry-derived visual lines plus a heading-aware region walk |
| `crates/da-parse/src/native_region_input.rs` | additive `build_native_word_region_carrier`, reusing the T01 carrier types |
| `crates/da-parse/src/lib.rs` | registration and re-exports |
| `crates/da-parse/src/native_segmentation.rs` | one-visibility change only: `full_marker` is now `pub(crate)` so the heading vocabulary cannot drift from S03's |

Invariants:

- **Content is conserved, never invented.** The split is a partition of the parent
  text's characters; `sum(word chars) + separator_chars == parent_char_len`, the
  reconstruction digest recomputes from the ordered word digests, and a non-closing
  allocation is the typed `ContentNotConserved`.
- **Geometry states its origin.** A single-word span keeps the parent box unchanged
  (`MeasuredParentSpan`); a multi-word span gets a `ProportionalEstimateFromParentSpan`
  box in its own labelled field, horizontal extent allocated by character count,
  vertical extent inherited. No estimate is presented as a measurement.
- **Font data is inherited, never synthesised** from the parent `LayoutToken`.
- **Lines come from geometry, not from the collapsed lattice.** Grouping is by
  baseline proximity and vertical overlap; ties on `(y_min, x_min)` fall back to the
  donor's document order, because a heading and the entry below it share a left margin.
- **A region is entered only through recorded evidence**: `PreambleDefault`,
  `NormalizedLine`, `OrdinalStrippedLine`, `LeadingSpanLine` (the line opens with a
  span that is exactly a vocabulary entry and the line looks like a heading), or
  `ProminentSectionHeading` (gated by `heading_max_words` and by measurement against
  the document's own modal body font size). A region that was never entered is a typed
  `RegionNeverEntered`, never an implied default.
- **No TEI or Java input, no wire-format change.** The S03 v1/v2 formats and the frozen
  D499 carriers (`data/parse/layout_bakeoff.yaml`, `data/parse/grobid_compatibility.yaml`,
  `artifacts/parse_compat/absorption_progress_baseline.json`) are byte-identical: they
  do not appear in `git status`.

## 3. Measurement on the three pinned PDFs

Detached run, no timeout wrapper, full log and real exit codes
(`/tmp/s04t04/{build.log,probe.log,rc.txt,run_env.txt}`; wall clock 12:24:59Z → 12:26:24Z;
`BUILD_RC=0`, `PROBE_RC=0`; digest `gsd_exec[1ffd2b0a-be17-415f-9c34-030569f7d937]`).

Each source PDF's sha256 was checked against its pin before and after the run and was
stable.

| document | spans | words | words with internal whitespace | header band | bibliography band |
|---|---|---|---|---|---|
| hub-2507.19457 | 14 118 | 40 395 | **0** | ok, 4 atoms | typed_absence |
| rlm-2512.24601 | 8 330 | 24 000 | **0** | ok, 4 atoms | ok, 34 entries / 2 field roles |
| reca-2605.26525v1 | 5 430 | 27 843 | **0** | refusal (see §4) | ok, 54 entries / 0 field roles |

`TokenContainsWhitespace` no longer occurs anywhere: the blocker is removed.

Honest regional outcomes:

| document | header | body | references |
|---|---|---|---|
| hub | 5 words, `PreambleDefault`, page 0 line 0, font 9 962 | 40 390 words, `ProminentSectionHeading`, page 0 line 1, font 17 215 | 0 words, typed absence |
| rlm | 3 words, `PreambleDefault`, page 0 line 0, font 17 215, bold | 5 934 words, `ProminentSectionHeading`, page 0 line 1, font 9 962, bold | 18 063 words, `NormalizedLine`, page 9 line 5, font 11 955, bold |
| reca | 2 words, `PreambleDefault`, page 0 line 0, font **0** | 6 366 words, `ProminentSectionHeading`, page 0 line 1, font 18 183, bold | 21 475 words, `NormalizedLine`, page 9 line 12, font 12 624, bold |

### Why hub has no References region

Diagnosed on real bytes with `--diagnose-markers`
(`gsd_exec[658ffb4a-e6e0-4d46-8632-da1fff314ec8]`), which re-derives each S03 region
run start and locates it in the heading-aware walk:

```
hub: S03RUN region=References page=70 source=92 derived_line=31
     line_words=13 line_bold=false line_font=8966 leading_source=90 is_leading=false
```

hub's only S03 "references" transition is a **mid-line, non-leading, body-font,
non-bold** span — the exact signature of the false-positive class the S03/T03 oracle
documented. Its max font equals hub's measured body font (8 966). Recovering that
region would require reinstating the defect T03 pinned. The typed absence is therefore
the honest outcome, and the same diagnosis shows the leading-span rule doing real work
elsewhere: rlm's body opens on a line whose leading span is a vocabulary entry
(`match=LeadingSpanLine`) while its mid-line S03 mark (page 0 source 40) is correctly
ignored, as is rlm page 21 source 92.

## 4. Residual blocker this repair does not own

`reca-2605.26525v1`: the header band refuses with
`InvalidFontSize { page: 0, source: 0, milli_pt: 0 }`.

The donor reports font size `0` for that span, and the T02 header band refuses
`milli_pt == 0` by contract. This is a donor-measurement fact, not a projection defect:
zero-font-size spans are common and were censused on all three documents — hub 113,
rlm 101, reca 135 — but reca is the only document where such a span is the **first**
header token.

This repair deliberately does **not** touch the T02 contract, does not invent a font
size, and does not silently drop the span. The open question — whether a donor-reported
zero should become a per-token typed disposition inside the header band instead of
aborting the band — belongs to the T02/header-band owner. Captured as
`PROBE_RESIDUAL` / `PROBE_RESIDUAL_COUNT=1`.

## 5. Verification

`cargo test -p da-parse` (all targets), `cargo clippy -p da-parse --all-targets
--no-deps -- -D warnings`, `cargo fmt -p da-parse -- --check` — see
`/tmp/s04t04/gate_*`. New coverage:

- `crates/da-parse/tests/native_word_projection_contract.rs` (9 tests): partition,
  geometry origin, determinism and sidecar-order independence, inherited style and
  per-word kind, conservation accounting, redaction, typed refusals, typed absences.
- `crates/da-parse/tests/native_word_projection_properties.rs` (6 properties):
  conservation, no internal whitespace, determinism, typed absence for empty and
  whitespace-only text, geometry origin versus word count, digest recomputation.
- `crates/da-parse/tests/native_heading_regions_contract.rs` (14 tests): line
  derivation, preamble/abstract behaviour, the max-words gate, the
  `ProminentSectionHeading` fallback, the leading-span rule and its inverse negative,
  vocabulary coverage for every S03 marker spelling, ordinal stripping, typed absences,
  redaction, config/identity refusals, and the end-to-end proof that the T02/T03 bands
  accept projected words.
- `crates/da-parse/tests/architecture_contract.rs`: the two new modules are gated for
  import purity, banned coupling markers, redaction, vocabulary anti-drift, module
  registration and shadow quarantine, with a negative control on the scanner.

GitNexus: the index is 21 commits behind HEAD, and `detect_changes --scope all` reports
7 touched symbols, 0 affected processes, `risk_level: low`; symbols in the two new
modules are not indexed yet, so that zero is not evidence of no impact. Caller analysis
for the touched surface was confirmed by text search instead.

## 6. Locks

`import_eligible=false`, `auto_promote=forbidden`, `deployed=false`,
`production_route=forbidden`, `graph_write=forbidden` (D127 / ADR-076 §14) are
untouched. No graph write, no import, no download, no promotion, no threshold
acceptance, no CLI/DB/state-store surface, no new dimension. No T04 receipt and no
T04 completion was written; the canonical `gsd_replan_task` for T04 already carries the
repair in its contract, and `gsd_task_complete` was not called.
