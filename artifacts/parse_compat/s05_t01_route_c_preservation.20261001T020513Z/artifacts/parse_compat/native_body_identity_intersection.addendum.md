# Addendum (bounded correction) — S05/T01 body-identity intersection

Milestone `M514-1lss2s` · Slice `S05` · Task `T01`.
Finding repaired: **`S05-T01-BODY-IDENTITY-INTERSECTION`** (HIGH: the adopted
`Body` boundary was a scalar `(page, source_ordinal)` interval, so equal
endpoints read as complete agreement and a `Header` parent inside the interval
could be adopted).

This file **adds** a bounded correction beside
`artifacts/parse_compat/native_body_boundary_grounding.evidence.md`
(sha256 `5c119ce74462081d17702f58ce36f921ffb0c948840b3e1d00a5c006e1f93d81`) and
the earlier `artifacts/parse_compat/native_body_additive_assignment.evidence.md`
(sha256 `6c229c9814cccb180580b0b960c3cbcbd79e7284ba0f698f80db22767c5ef93f`). It
does **not** edit, replace or erase either document; both are preserved
byte-for-byte. The v5 source/artifact/evidence set relevant to this repair (881
files, including both prior evidence documents, the probe, the contracts and the
run directories) was copied and hashed before the first edit into
`artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/`,
`MANIFEST.sha256` sha256
`55963c8ca3d1f540623b85021bb1f099dec0207820a9f8989242c6cf588e0089`. Three files
this repair does not touch (`crates/da-parse/src/lib.rs`,
`crates/da-parse/tests/architecture_contract.rs`, `crates/da-cli/Cargo.toml`) are
proven SAME by the producing runs' own `inputs.before.sha256` /
`inputs.after.sha256` sets (see §6).

No engine, dependency, download, graph write, import promotion or Java
activation was introduced; no threshold was accepted; routing and locks are
unchanged. The canonical abort/recovery record was **not** touched, resumed,
settled or made terminal, and the runs did not stage, commit, reset or stash
anything. `T01` remains `NotAssessable` on the real corpus; this addendum claims
only the bounded intersection correction and does not claim task completion or
recovery.

## 1. What was wrong, and what changed

The boundary arbitration in `build_native_body_assignment` treated a route's
`Body` extent as the scalar tuple interval `[lower, upper]` with
`lower = segmentation.0.max(heading.0)`, `upper = segmentation.1.min(heading.1)`
and admitted a heading token when `identity >= lower && identity <= upper`.
Five real defects followed from that:

1. `source_ordinal` is the donor's extraction identity and **may be permuted
   against the admitted reading order** (`LayoutPage::validate` says so
   explicitly), so the tuple interval is not the set either route carries and is
   not in reading order.
2. `segmentation == heading` compared only the **first and last** identity, so
   **equal endpoints with a different interior** read as complete agreement.
3. Only the two endpoints were checked against the S03 region, so a **`Header`
   parent inside the interval** was adopted as body.
4. Two **disjoint** identity sets whose scalar intervals merely overlapped were
   adopted.
5. One parent span may project **several words**; interval filtering is not a
   per-parent conservation and can drop or keep part of one parent.

The bounded repair (all of it in `crates/da-parse/src/native_body_text.rs`):

* the comparison universe is the **word-bearing parent identity set** —
  `projection.parents` `(page_ordinal, source_ordinal)` — because only a parent
  that produces a word can appear on the word-level heading route;
* `segmentation_body_ids` is the set of S03 `Body` identities,
  `segmentation_word_body_ids` is that set restricted to the word-bearing
  universe, and `heading_body_ids` is the heading route's identity set;
* **complete membership agreement** is
  `segmentation_word_body_ids == heading_body_ids`; anything else takes the set
  **intersection**, and an empty intersection is the named refusal
  `RoutesDisagree` — never a pick;
* the adopted tokens are the heading tokens whose parent identity is in the
  intersection, **in the heading route's admitted reading order**; every adopted
  token's parent identity is re-checked against `segmentation_body_ids`, so **all
  adopted tokens are corroborated `Body`**, not just the ends;
* `adopted_bounds` is the first and last adopted parent identity in that reading
  order (it may be non-monotonic under a permuted `source_ordinal`), and the body
  `page_span` is measured from the adopted tokens rather than the unfiltered run;
* the helper was renamed `region_token_identity` -> `parent_identity_of` and
  documents the word->parent mapping: on the word route `source_ordinal` is the
  parent span's extraction identity and `location.token_ordinal` is the word's
  reading position, so the parent identity is never the word ordinal and is never
  compared by scalar tuple order;
* public types did **not** change shape (no report/API rewrite): the same
  `NativeBodyAssignment` fields are populated, with corrected values and corrected
  doc comments on `BodyAssignmentRoute::BothRoutesIntersection`,
  `BodyBoundaryVerification`, and `NativeBodyAssignment::adopted_bounds`.

`crates/da-parse/src/lib.rs`, `crates/da-cli/examples/native_body_character_probe.rs`,
`crates/da-cli/Cargo.toml`, the uncertainty-split contract and the architecture
contract are **byte-identical** to before this repair (their captured hashes are
unchanged, see §6). No S03/S04 carrier, no `data/parse/*.yaml`, no Java, and no
threshold was touched.

## 2. Preservation, and the §2 artifact comparison (run1 vs run2, separately)

### 2.1 The prior chain (history, quoted from its own preserved files; not rewritten)

The prior boundary-grounding repair recorded two runs, and their artifact
comparisons are **different facts that must not be collapsed into one**:

| prior run | `hash_compare.txt` | artifact comparison |
|---|---|---|
| `s05_t01_body_boundary_grounding_run/` | `SAME source`, `SAME input`, `SAME preserved` | **`CHANGED artifact`** `14f76bba…` -> `68835c94…` |
| `s05_t01_body_boundary_grounding_run2/` | `SAME source`, `SAME input`, `SAME preserved` | **`SAME artifact`** `68835c94…` -> `68835c94…` |

Both files are preserved byte-for-byte (copied into the v5 preservation set in
2.0 above and left in place). The prior evidence's §2 sentence "CHANGED artifact"
is run1's comparison; run2's comparison is `SAME`. They are recorded separately
here rather than editing the prior document.

### 2.2 This repair's two runs

| run | source/input | report before | report after | artifact comparison |
|---|---|---|---|---|
| `s05_t01_body_identity_intersection_run1/` | `source_inputs: SAME` | `68835c94cf38367e9855b581418a0615db425bedbe57a247a2200ba5aac91bc9` | `68835c94cf38367e9855b581418a0615db425bedbe57a247a2200ba5aac91bc9` | **`SAME artifact`** |
| `s05_t01_body_identity_intersection_run2/` | `source_inputs: SAME` | `68835c94cf38367e9855b581418a0615db425bedbe57a247a2200ba5aac91bc9` | `68835c94cf38367e9855b581418a0615db425bedbe57a247a2200ba5aac91bc9` | **`SAME artifact`** |

The regenerated `artifacts/parse_compat/native_body_character_scope_bound.json`
is byte-identical to the preserved v5 report in **both** runs: the three pinned
documents do not exercise the corrected case (their word-bearing `Body` identity
intersection already equals the old scalar interval), so a real-data regression
is neither observed nor possible to claim. The correction is proven by the five
new oracle negatives in §4, not by a report diff. This is stated as the measured
fact; no `CHANGED` comparison is claimed where none occurred.

### 2.3 The v4 source bytes are lost (disclosure, not restoration)

The **rejected v4 worktree source bytes are not recoverable**. The interrupted
auto-commit left only an earlier 559-line partial snapshot that does **not**
contain `build_native_body_assignment`:

* `artifacts/parse_compat/s05_t01_body_boundary_grounding_run/preserved_before/native_body_text.rs`
  — 559 lines, sha256
  `49c286b0e3ea46c6e4735d55962a4b5ba1366308427608fb29c24c5e9785d992`, 0
  occurrences of `build_native_body_assignment`.
* the v4 measurement itself is preserved as
  `.../preserved_before/native_body_character_scope_bound.v4.json`
  (schema `native-body-character-scope-bound.v4`, sha256
  `14f76bba14926ba33cf1676a4e354f4e82144b3940b771109851ba4a93693192`).

That partial snapshot is **not** restored, renamed or presented as v4 source; the
loss is named and left as a loss.

## 3. Run (exact metadata; original rc; no execution timeout)

Capture script sha256
`bd21387f23e182325aeba7e148c6a1da536b6d07dd474bfd8a6751c1c4b8878c` (identical at
start and after each run, recorded in each `RUN.identity.txt`). Each run is a
fresh, separate directory; none is appended to or reused. `git_head =
a6facbef77a7a53574883dd451270fa87d2891e0`, `cargo 1.98.1`, `rustc 1.98.1`.

A first run on the pre-format source failed **only** `cargo fmt --all -- --check`
(rc 1, one `native_body_text.rs` reflow); it is preserved immutably and separately
as `s05_t01_body_identity_intersection_run1.FAILED_fmt.20261001T012725Z/` and is
not a producing run. `cargo fmt --all` was then applied and the two producing runs
below were started from the formatted source.

run1 window `2026-10-01T01:28:55.831950892Z` -> `2026-10-01T01:32:25.479951185Z`:

| step | command | elapsed ms | rc |
|---|---|---|---|
| 01 | `cargo test -p da-parse --test native_body_character_oracle` | 2098 | 0 |
| 02 | `cargo test -p da-parse --test native_body_uncertainty_split_contract` | 1660 | 0 |
| 03 | `cargo test -p da-parse --test architecture_contract` | 2931 | 0 |
| 04 | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 17320 | 0 |
| 05 | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 23852 | 0 |
| 06 | `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` | 23213 | 0 |
| 07 | `cargo run -p da-cli --features pdf-layout --example native_body_character_probe` | 107393 | 0 |
| 08 | `cargo fmt --all -- --check` (LAST Rust action) | 30498 | 0 |

run2 window `2026-10-01T01:32:25.713245506Z` -> `2026-10-01T01:34:13.657179760Z`:
the same eight commands, all rc 0 (2670 / 577 / 1107 / 1037 / 1109 / 931 / 71687 /
28109 ms). Every per-step `*.argv` / `*.meta` / `*.rc` / `*.stdout` / `*.stderr`
is durable in the run directory; `08_fmt_check.stdout` and `.stderr` are both 0
bytes (clean). The ~15 295 B clippy stderr on steps 05/06 is vendored
`samyama-graph` warning noise outside this change; `-D warnings` returned rc 0 on
both changed crates.

## 4. Measured result

Test totals from the captured stdout (identical in both runs): **52 passed**
(oracle, was 47 — the five new negatives below), **11 passed** (uncertainty
split), **101 passed** (architecture). The five new oracle negatives, each green:

| negative | what it pins |
|---|---|
| `a_permuted_source_ordinal_keeps_the_identity_intersection` | reading order differs from `source_ordinal`; the identity set still agrees and the adopted stream is `introduction alpha beta gamma`; the old scalar filter refuses every token |
| `equal_extent_endpoints_with_a_different_interior_are_not_whole_run_agreement` | equal first/last identities over a different interior is `ConservativeExtentIntersection`, not `TwoIndependentRoutesAgree` |
| `a_header_parent_inside_the_extent_is_never_adopted` | a `Header` parent inside the interval contributes no word to the body |
| `disjoint_body_identity_sets_are_a_named_refusal` | overlapping scalar intervals with disjoint identity sets is `RoutesDisagree`, no body |
| `a_duplicated_parent_contributes_all_or_none_of_its_words` | a multi-word parent contributes all of its words when corroborated and none when not |

One existing expectation was corrected with the semantics: the shared-start /
different-end test now asserts `adopted_bounds == ((0, 2), (0, 2))` (the marker
route's last `Body` token is a whitespace separator that projects no word) instead
of the scalar interval `((0, 2), (0, 3))`.

Real three pinned PDFs (probe step 07; the report is the preserved v5 artifact
`68835c94…`):

| document | segmentation Body tokens | heading Body tokens | ends (seg / heading) | verification | adopted | body gap |
|---|---|---|---|---|---|---|
| hub-2507.19457 | 0 | 40 390 | `None` / `(95,55)` | `single_route_uncorroborated` | none (refused) | not_assessable |
| rlm-2512.24601 | 4 681 | 5 934 | `(42,5)` / `(9,4)` | `conservative_extent_intersection` | intersection `(0,40)`–`(9,4)`, route `both_routes_intersection`, 5 742 word tokens | not_assessable |
| reca-2605.26525v1 | 0 | 6 366 | `None` / `(9,50)` | `single_route_uncorroborated` | none (refused) | not_assessable |

## 5. What this proves, and what stays unresolved (no clearance)

**Proven.** The adopted `Body` boundary is now a word-bearing **parent identity
set intersection** computed from both routes, in admitted reading order; complete
membership is the only whole-run agreement; every adopted token — not just the
ends — must be corroborated `Body` by the S03 assignment stream; a permuted
`source_ordinal`, an equal-endpoint-different-interior pair, a foreign `Header`
interior parent, disjoint identity sets, and a multi-word parent are all handled
without a scalar tuple interval.

**Still unresolved, named (no invented clearance).**

1. `T01` remains `NotAssessable` on all three pinned documents: `absent_from_plane
   = 0` everywhere, so there is no proven content loss,
   `d498_reopen_required = false`, and no D498 decision is recorded. The body-gap
   question is not closed.
2. The additive population is 1/3 (only rlm corroborates both routes); hub and
   reca have no marker `Body` run at all and remain named refusals.
3. Locks unchanged: `claim_scope = diagnostic_only`,
   `product_coverage_effect = none`, `promotion_state = candidate`,
   `auto_promote = forbidden`, `graph_mutation = forbidden`,
   `import_eligible = false`; `thresholds = not_accepted`.
4. The three pinned documents do not exercise the corrected case, so the real
   report is byte-identical; the correction is proven only by the synthetic
   negatives in §4.

## 6. Bounded diff (source bytes)

| file | before this repair | after this repair | state |
|---|---|---|---|
| `crates/da-parse/src/native_body_text.rs` | `be375e3bf7c82d4382645e46e28c70c91d0864542b4e6065974e310027875d93` | `29e25e1d17030a3baf064c396866eeddaa16ab133a9d244aed26f0390b39adec` | CHANGED (the fix) |
| `crates/da-parse/tests/native_body_character_oracle.rs` | `4d8cae032f5756cceb11c5c12b59f404d9a99dbd48dab8f530b1eaa2b746e487` | `1440787f5d73cbab4e70bb132f872c6b3b29b46e24f7e9f5eab83a7c70f855fc` | CHANGED (5 negatives + helper) |
| `crates/da-parse/src/lib.rs` | `c62cd8f29865eed679f436349e3098fa3eed1ddcc7b0112e3cb3948716c83285` | same | SAME |
| `crates/da-parse/tests/native_body_uncertainty_split_contract.rs` | `34eb794d25fa3ee544cfa9b3149d54292bfb177926f9079039b88cc31f7cf7cf` | same | SAME |
| `crates/da-parse/tests/architecture_contract.rs` | `6e951a200926520198117eaffdab82d925f54b41116fb6106df8650120b09821` | same | SAME |
| `crates/da-cli/examples/native_body_character_probe.rs` | `fe61838528aab9fdd8a13fb0c3a3e48b29d0353606a7e0aa45d9ae374aa78300` | same | SAME |
| `crates/da-cli/Cargo.toml` | `5885062267e534d015e68df6cf32bdc91630faa55d48dffaa0bcc4c296d94a42` | same | SAME |
| `artifacts/parse_compat/native_body_character_scope_bound.json` | `68835c94cf38367e9855b581418a0615db425bedbe57a247a2200ba5aac91bc9` | same | SAME (regenerated) |

Preserved S03/S04 acceptance pins, unchanged:
`native_body_character_scope_bound.accepted.v3.json`
`027d8c934e9009001b968ecb95d13eef586d3740a83a2884d21924a49438f27a`,
`native_body_order_lineage.json`
`3b08509c6b3b2be84db1b7ab99e41c9c4bd1bf88a68b754c5c3624b22f302395`,
`native_body_uncertainty_split.json`
`478054d5bd36d6d59057d53f4ba6ddbc21ccfb5578241fd081b595c6c877d69b`,
`native_body_character_shadow.json`
`55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361`,
`data/parse/layout_bakeoff.yaml`
`530ce05297e881ac91b7404c608dec5aadd9ffd22b7bbbbdd68e064b534290b5`,
`crates/da-parse/tests/fixtures/preserved/provenance.yaml`
`028a5d62cc8412097ea03ee5e529bae67bf8f9d4fe7819116e6365ad2aca0ebf`.

## 7. Runs preserved separately (immutable, not reused)

| directory | state |
|---|---|
| `s05_t01_body_identity_intersection_run1.FAILED_fmt.20261001T012725Z/` | pre-format source; steps 01–07 rc 0, step 08 (`cargo fmt --all -- --check`) rc 1; preserved byte-for-byte, not a producing run |
| `s05_t01_body_identity_intersection_run1/` | producing run: all eight steps rc 0, `source_inputs SAME`, `report_artifact SAME` |
| `s05_t01_body_identity_intersection_run2/` | producing run: all eight steps rc 0, `source_inputs SAME`, `report_artifact SAME` |
| `s05_t01_v5_preservation.20261001T011654Z/` | pre-edit v5 sources/artifact/evidence copy, 881 files, `MANIFEST.sha256` sha256 `55963c8c…` |
