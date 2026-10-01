# S05/T01 — additive native Body extraction/assignment, run evidence

Milestone `M514-1lss2s` · Slice `S05` · Task `T01`.
Producing run: `artifacts/parse_compat/s05_t01_additive_assignment_run/capture.sh`
(detached, sequential, no execution timeout).
Report: `artifacts/parse_compat/native_body_character_scope_bound.json` (schema
`native-body-character-scope-bound.v4`).
Durable run directory: `artifacts/parse_compat/s05_t01_additive_assignment_run/`
(`capture.sh`, per-step `*.argv` / `*.meta` / `*.rc` / `*.stdout` / `*.stderr`,
`steps.tsv`, `hashes.*.txt`, `hash_compare.txt`, `RUN.identity.txt`).

Every number below is a byte-level fact of this capture run, not a restatement of
an earlier document. No engine, dependency, download, graph write, import
promotion or Java activation was introduced; no threshold was accepted; routing
and locks are unchanged. The canonical abort/recovery record was **not** touched,
resumed, settled or made terminal, and the run did not stage, commit, reset or
stash anything.

## 1. What the repair changed (real, not a report)

The P2 finding is that the S03 marker segmentation (`segment_document_full`)
enters `FullSemanticRegion::Body` only through a textual marker — a numbered
section heading (`heading_depth(text) > 0`) or a `full_marker` vocabulary entry.
A document whose section headings are unnumbered and outside that vocabulary
never enters `Body` and is reported as `NativeBodyTextError::NoBodyTokens`. That
is a **route limitation**, not evidence that the document has no body.

This repair adds a bounded, additive, same-engine Body extraction/assignment path
and an independent boundary check. It does **not** modify the byte-pinned
S03/S04 carriers, does not rewrite the segmentation, does not read TEI as native
input, and does not switch the primary route.

* `crates/da-parse/src/native_body_text.rs`
  * `NATIVE_BODY_ASSIGNMENT_SCHEMA_VERSION` = `da-parse.native-body-assignment/v1`.
  * `BodyAssignmentRoute { SegmentationMarker, HeadingGeometry }`.
  * `BodyBoundaryVerification { TwoIndependentRoutesAgree,
    SegmentationBodyAbsentHeadingGrounded, HeadingBodyAbsentSegmentationGrounded,
    RoutesDisagree, NoRouteEnteredBody }`.
  * `NativeBodyAssignment` (both route token counts and start identities, the
    adopted route, the recorded `HeadingMatch`, the verification, and the adopted
    `NativeBodyText` or `None`).
  * `build_native_body_assignment(document, lattice, lexemes, segmentation, join,
    rung)`: builds the marker route with `build_native_region_carrier` and the
    geometry route with `project_native_words` -> `assign_heading_regions` ->
    `build_native_word_region_carrier`; compares the two first Body tokens by the
    stable `(page_ordinal, source_ordinal)` identity; when the marker route is
    absent it requires (a) the recorded heading entry evidence and (b) the
    boundary token to exist in the S03 assignment stream and not be page
    furniture, else it returns `BodyBoundaryUngrounded`; when both routes exist but
    start differently it withholds the boundary (`RoutesDisagree`, no adopted
    body). Two typed error variants were added: `HeadingRoute(String)` and
    `BodyBoundaryUngrounded`.
  * The existing ordered, occurrence-conserving, token-boundary-grounded
    alignment (`ordered_occurrence_conserving_token_boundary_alignment_v2`) and
    the P1 plane verdict path are unchanged; the P2 body band consumes the
    adopted body through the same `classify_body_coverage`.
* `crates/da-parse/src/lib.rs` — exports the new names.
* `crates/da-parse/tests/native_body_character_oracle.rs` — four additive tests
  (heading-grounded adoption when the marker route is absent; withheld boundary on
  route disagreement; agreement when both routes start at the same token; typed
  absence when neither route enters Body).
* `crates/da-parse/tests/architecture_contract.rs` — the S05/T01 purity contract
  now also requires `pub fn build_native_body_assignment` and
  `BodyBoundaryVerification` on the same TEI-free surface.
* `crates/da-cli/examples/native_body_character_probe.rs` — a new
  `additive_assignment` band and aggregate (schema v4). The additive assignment
  is never `ContentLoss`; every unresolved or withheld boundary is
  `NotAssessable`/typed absence.

## 2. Preservation (all unchanged before == after; the run writes none of them)

| artifact | sha256 | state |
|---|---|---|
| `artifacts/parse_compat/native_body_character_scope_bound.accepted.v3.json` | `027d8c934e9009001b968ecb95d13eef586d3740a83a2884d21924a49438f27a` | accepted v3 report copied byte-identically before the edit; untouched by the run |
| `artifacts/parse_compat/native_body_character_shadow.json` | `55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361` | historical diagnostic, untouched |
| `artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json` | `ce87e3af6fc9f332520df77463001746aecd76fc4a82d949b1974b28d2ae9d3f` | first-repair artifact, untouched |
| `artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md` | `a00a761d2716a21c0a83b71ed92146eb79d1278cdfba925c503aedba02831982` | first-repair evidence, untouched |
| `artifacts/parse_compat/native_body_character_scope_bound.repair2_artifact.json` | `1ad2b56f653f0ea22784f9a3c975556ee7771b0f0b365ec169f7914624661638` | second-repair artifact, untouched |
| `artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md` | `6560eb028b573260f58b6dd23bc97e7df733aa880c85c9831fc492fafd9bc67c` | second-repair evidence, untouched by this run |
| `data/parse/layout_bakeoff.yaml` | `530ce05297e881ac91b7404c608dec5aadd9ffd22b7bbbbdd68e064b534290b5` | declaration pin, SAME before/after |
| `crates/da-parse/tests/fixtures/preserved/provenance.yaml` | `028a5d62cc8412097ea03ee5e529bae67bf8f9d4fe7819116e6365ad2aca0ebf` | oracle pin, SAME before/after |
| hub / rlm / reca TEI | `98546123…` / `e6aa41f5…` / `dac18882…` | oracle bytes, SAME before/after |
| hub / rlm / reca PDF | `ab3a5139…` / `8567362c…` / `a5672497…` | pinned inputs, SAME before/after |

The three PDFs and three preserved TEI fixtures were re-hashed from their bytes
by the probe and refused on any drift before any body was assembled.

## 3. Run (exact metadata; original exit codes; no execution timeout)

Capture script sha256 `871cff4126f4658dcd1ac360dab8cdad4b4ba70bbc96362abf23c41ec955e61e`
(identical at start and after the run, `RUN.identity.txt`). Run window (UTC)
`2026-10-01T00:25:53.517395753Z` → `2026-10-01T00:29:16.948837653Z` (~204 s).
`git_head = a6facbef77a7a53574883dd451270fa87d2891e0`, `cargo 1.98.1`,
`rustc 1.98.1`.

| step | command | elapsed ms | rc | stdout B | stderr B |
|---|---|---|---|---|---|
| 01 | `cargo test -p da-parse --test native_body_character_oracle` | 20125 | 0 | 2997 | 258 |
| 02 | `cargo test -p da-parse --test native_body_uncertainty_split_contract` | 1607 | 0 | 811 | 277 |
| 03 | `cargo test -p da-parse --test architecture_contract` | 2445 | 0 | 7600 | 243 |
| 04 | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 15527 | 0 | 0 | 140 |
| 05 | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 22681 | 0 | 0 | 15295 |
| 06 | `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` | 20422 | 0 | 0 | 15295 |
| 07 | `cargo run -p da-cli --features pdf-layout --example native_body_character_probe` | 94147 | 0 | 1886 | 15258 |
| 08 | `cargo fmt --all -- --check` (LAST Rust action) | 26169 | 0 | 0 | 0 |

Test totals from the captured stdout: **43 passed** (oracle, 39 pre-existing + 4
additive), **11 passed** (uncertainty split), **101 passed** (architecture).
The 15 295 B stderr on the clippy and probe steps is vendored `samyama-graph`
warning noise outside this change (the earlier runs in this repo show the same
figure); a scan of those captures for a warning naming `da-parse` or `da-cli`
returns 0 matches.

## 4. Hash bindings (source, input and artifact SAME before == after)

`hash_compare.txt`: `SAME source`, `SAME input`, `SAME artifact`. Sources under
repair (after == before, from `hashes.source.*.txt`):

| path | sha256 |
|---|---|
| `crates/da-parse/src/native_body_text.rs` | `3eeb4d5b08143851015715c378359afd3628ae2485d60ed7179bb0677d3e22a9` |
| `crates/da-parse/src/lib.rs` | `fef194971213a9313bbcb3240fd848a18823a04837f47e64d067f3819fb28287` |
| `crates/da-parse/tests/native_body_character_oracle.rs` | `7c03cb359d374210d988f38701be78b33671b8a832f3d43eaa5ebc8abab810c3` |
| `crates/da-parse/tests/native_body_uncertainty_split_contract.rs` | `34eb794d25fa3ee544cfa9b3149d54292bfb177926f9079039b88cc31f7cf7cf` |
| `crates/da-parse/tests/architecture_contract.rs` | `6e951a200926520198117eaffdab82d925f54b41116fb6106df8650120b09821` |
| `crates/da-cli/examples/native_body_character_probe.rs` | `e04fba353aad8af4ab0cc87f5c83ac017c0165518ed40a2775f3ced0856b7f4a` |
| `crates/da-cli/Cargo.toml` | `5885062267e534d015e68df6cf32bdc91630faa55d48dffaa0bcc4c296d94a42` |

Final report `artifacts/parse_compat/native_body_character_scope_bound.json` =
`14f76bba14926ba33cf1676a4e354f4e82144b3940b771109851ba4a93693192` (schema v4,
native surface `da-parse.native-body-text/v5`). The run re-produced these bytes
exactly over the same sources (`SAME artifact`), so the v4 report is
byte-reproducible; the accepted v3 bytes survive at the `.accepted.v3.json` path.

## 5. Measured result (additive band)

| document | segmentation Body tokens | heading Body tokens | first Body token (seg / heading) | verification | adopted | body gap |
|---|---|---|---|---|---|---|
| hub-2507.19457 | 0 (typed `NoBodyTokens`) | 40 390 | `None` / `(0,1)` | `segmentation_body_absent_heading_grounded` | `heading_geometry` | `not_assessable` |
| rlm-2512.24601 | 4 681 | 5 934 | `(0,40)` / `(0,1)` | `routes_disagree` | none (withheld) | none |
| reca-2605.26525v1 | 0 (typed `NoBodyTokens`) | 6 366 | `None` / `(0,1)` | `segmentation_body_absent_heading_grounded` | `heading_geometry` | `not_assessable` |

Additive aggregate: 3 declared / 2 contributing / 1 typed absence
(`rlm-2512.24601`, reason `RoutesDisagree`); alignment 54 610 oracle units, 431
exact in plane, 401 exact in body, 0 boundary tiles, 54 078 present-in-token +
86 spanning + 15 material uncertain, **0 absent**, verdict `not_assessable`,
`d498_reopen_required = false`, `thresholds = not_accepted`.

## 6. What this proves, and what stays unresolved (no clearance)

**Proven / repaired (P2 localization for HUB and RECA).** The `NoBodyTokens`
typed absence for hub and reca is **localized to the marker route**: the
segmentation never enters `Body` because neither document carries a numbered
section heading or a `full_marker` vocabulary entry. The independent
heading/geometry route does enter `Body`, and its boundary is grounded in the
recorded heading entry evidence and cross-checked against the S03 assignment
stream. The additive band therefore assigns a real body for both documents
(40 390 and 6 366 tokens) where the previous state had only a typed absence. This
is the concrete engineering repair, not a diagnostic restatement.

**Unresolved and named (no invented clearance).**

1. **rlm: the two native routes disagree on the body start.** The marker route
   enters `Body` at source token `(0,40)` (the first numbered heading); the
   heading route enters at `(0,1)`, because `heading_marker` maps `"abstract"` to
   `Body` while the segmentation's `full_marker` does not. This is the known
   early Header/Body boundary defect, now located to an exact token and cause. The
   additive verification **withholds** the boundary (`RoutesDisagree`) rather than
   picking one, so rlm contributes no additive body. Next engineering fix: an
   independent boundary arbitration (for example adopting the narrower region
   corroborated by both routes, or aligning the two native vocabularies on
   whether the abstract is Header) — deliberately **not** invented here.
2. **P1 plane is unchanged and still `NotAssessable` for all three documents.**
   `absent_from_plane = 0` everywhere, with 69 400 in-token + 92 spanning + 17
   material uncertain occurrences at corpus level. `GranularityOnly` is correctly
   withheld (a zero-absent plane is not preservation), and there is **no proven
   content loss**, so `d498_reopen_required = false` and no D498 decision is
   recorded. The body-gap question is **not** closed.
3. **The S03 parent-span band is unchanged** (hub/reca remain typed
   `NoBodyTokens` there; the accepted v3 measurement is preserved). The additive
   band is a separate, additive assignment axis; it does not rewrite the S03
   receipt.
4. Locks unchanged: `claim_scope = diagnostic_only`,
   `product_coverage_effect = none`, `promotion_state = candidate`,
   `auto_promote = forbidden`, `graph_mutation = forbidden`,
   `import_eligible = false`; `thresholds = not_accepted`.

## 7. Negative / boundary cases covered by the tests in this run

* additive assignment adopts a body only when the sole entering route is grounded
  (`SegmentationBodyAbsentHeadingGrounded`);
* a disagreeing boundary is withheld, never picked (`RoutesDisagree`, no body);
* both routes starting at the same token identity is the corroborated case
  (`TwoIndependentRoutesAgree`);
* neither route entering Body is a typed absence (`NoRouteEnteredBody`), never a
  zero-length body;
* pre-existing axis-1 negatives remain green: equal-length different content,
  duplicate/surplus masking a selective loss, reordering, arbitrary substring in
  token, repeated occurrences not excused by one candidate, rung/case mismatch
  refusals, greedy-anchor conservatism, empty oracle, same-generator oracle.

## 8. Failed runs preserved separately (not overwritten)

| directory | cause | state |
|---|---|---|
| `artifacts/parse_compat/s05_t01_additive_assignment_run.FAILED_repo_path.20261001T001823Z/` | capture script resolved its repo root four levels up (`/root`), so every cargo step ran outside the workspace (rc 101, `could not find Cargo.toml`); no source, artifact or git state mutated | preserved byte-for-byte |
| `artifacts/parse_compat/s05_t01_additive_assignment_run.FAILED_fmt.20261001T002506Z/` | `cargo fmt --all -- --check` rc 1 (formatting drift in the four touched files only); all prior steps rc 0 | preserved byte-for-byte |

The final run uses a byte-identical capture script (`871cff41…`) and
`hash_compare.txt` shows `SAME source` / `SAME input` / `SAME artifact`.
