# S05/T01 — body-boundary grounding and the P1/P2 evidence axes, run evidence

Milestone `M514-1lss2s` · Slice `S05` · Task `T01`.
Review finding repaired: `S05-T01-BODY-BOUNDARY-GROUNDING` (third bounded repair;
request changes — the technical capture was green but the boundary logic was
still substantive-deficient).
Producing run: `artifacts/parse_compat/s05_t01_body_boundary_grounding_run2/capture.sh`
(detached, sequential, no execution timeout; the script that ran is preserved and
hashed in that directory). The earlier
`artifacts/parse_compat/s05_t01_body_boundary_grounding_run/` is an intermediate
run of the same repair before the final occurrence-conservation test was added;
it is preserved byte-for-byte and is **not** the producing run.
Report: `artifacts/parse_compat/native_body_character_scope_bound.json` (schema
`native-body-character-scope-bound.v5`; sha256 `68835c94cf38367e9855b581418a0615db425bedbe57a247a2200ba5aac91bc9`).

Every number below is a byte-level fact of the capture run, not a restatement of
an earlier document. No engine, dependency, download, graph write, import
promotion or Java activation was introduced; no threshold was accepted; routing
and locks are unchanged. The canonical abort/recovery record was **not** touched,
resumed, settled or made terminal, and the run did not stage, commit, reset or
stash anything.

## 1. What the review found, and what was changed

### 1.1 The boundary was not independently grounded

`build_native_body_assignment` compared only the **first** `Body` token of the
two native routes. It adopted the whole heading run whenever the starts matched,
and it accepted a single-route boundary when the heading route's own
`HeadingMatch` existed and the S03 token was merely **not `PageFurniture`** —
which let a token labelled `Header` pass. Both were real defects:

* a shared start with a different end was published as whole-run agreement;
* a single route's own entry evidence was treated as independent corroboration
  of itself;
* `Header` was an allowed boundary region.

The repair:

* `BodyBoundaryVerification` now names the honest outcomes:
  `TwoIndependentRoutesAgree` (first **and** last identity agree),
  `ConservativeExtentIntersection` (both routes enter `Body`, extents differ, the
  adopted run is the identity intersection both carry),
  `SingleRouteUncorroborated` (a named refusal — no body),
  `RoutesDisagree` (no overlap — no body), `NoRouteEnteredBody` (typed absence).
* The two old `*Grounded` variants are removed: a single-route boundary adopts
  **nothing**.
* Every adopted extent must be labelled `Body` in the S03 assignment stream at
  **both** ends; `Header` and `PageFurniture` are refused
  (`BodyBoundaryUngrounded`).
* `NativeBodyAssignment` now records `segmentation_end`, `heading_end` and
  `adopted_bounds`, so the extent agreement is inspectable, not implied.
* `BodyAssignmentRoute` gains `BothRoutesIntersection` for the case where the
  adopted extent is a strict sub-run of both routes.

### 1.2 The join policy was declared but never checked

`NativeBodyText` now records the join policy it actually applied
(`pub join: BodyJoinPolicy`), set by `from_carrier` (verbatim), `from_fragments`
and `from_joined_text` (the caller's declared policy). `classify_body_coverage`
refuses a native scope whose declared `join_policy` is not the policy the body
applied (`BodyGapReason::ScopeJoinMismatch`, `NotAssessable`). A
`VerbatimTokenConcat` scope over a body joined with
`SingleSpaceBetweenWordTokens` describes a different text and is no longer
believed. This closed a latent probe inconsistency where the additive band
selected a verbatim scope for a body joined with word spaces.

### 1.3 P1 and P2 are now two published axes, not one prioritised reason

`classify_body_coverage` previously short-circuited: a P1 plane uncertainty
returned before the P2 body evidence was ever read, so a body miss could not be
seen. Now:

* `BodyRegionVerdict` + `classify_body_region` derive the P2 body axis
  independently;
* `BodyCoverageGap` carries both `plane_verdict` (`BodyPlaneVerdict`) and
  `body_verdict` (`BodyRegionVerdict`);
* `BodyCoverageAggregate` carries both folded axes, each withholding a clean
  reading when the declared population is incomplete
  (`BodyGapReason::PopulationIncomplete`);
* the combined `verdict`/`reason` still prefers the plane axis when it is not
  clean (a plane loss or uncertainty cannot be improved by the body axis), but
  the body axis is published beside it, so nothing is disguised.

### 1.4 The P2 deficit is explicit and occurrence-conserving

`BodyContentAlignment::in_plane_not_in_body` was
`oracle_units - body.exact - body.boundary`, which folded *unbound* occurrences
into the deficit. It is now computed by joining the two per-occurrence outcome
vectors (the same oracle occurrence on both sides, index-aligned) and counting
exactly the occurrences that are **present in the plane and absent from the
body**. An occurrence absent from the plane as well belongs to P1; an occurrence
present but unbound is uncertainty, not a deficit. The locator was refactored to
produce `OccurrenceOutcome` per occurrence and fold it, so every occurrence is
consumed exactly once and a global side token count is never used as presence or
loss. No threshold and no new engine were added.

### 1.5 Bounded diff

Only `crates/da-parse/src/native_body_text.rs`, `crates/da-parse/src/lib.rs`
(exports), `crates/da-parse/tests/native_body_character_oracle.rs`, and
`crates/da-cli/examples/native_body_character_probe.rs` (scope selection + report
schema v5) changed. `crates/da-parse/tests/native_body_uncertainty_split_contract.rs`,
`crates/da-parse/tests/architecture_contract.rs` and `crates/da-cli/Cargo.toml`
are byte-identical to before (their hashes are the same before and after). No
S03/S04 primary carrier, no `data/parse/*.yaml`, no Java, no threshold was
touched.

## 2. Preservation (accepted prior history and frozen inputs)

`hash_compare.txt`: `SAME source`, `SAME input`, `SAME preserved`, `CHANGED
artifact` (the probe report is intentionally regenerated v4 → v5).

| artifact | sha256 | state |
|---|---|---|
| `native_body_character_scope_bound.accepted.v3.json` | `027d8c934e9009001b968ecb95d13eef586d3740a83a2884d21924a49438f27a` | accepted v3, untouched |
| `native_body_character_scope_bound.evidence.md` | `de77ab537f6d46e34c2697034fc5f7ce8548d1e305eb640a7ac8bfd8f93aa2dc` | prior evidence, untouched |
| `native_body_character_scope_bound.prior_repair.json` | `ce87e3af6fc9f332520df77463001746aecd76fc4a82d949b1974b28d2ae9d3f` | first repair, untouched |
| `native_body_character_scope_bound.prior_repair.evidence.md` | `a00a761d2716a21c0a83b71ed92146eb79d1278cdfba925c503aedba02831982` | first-repair evidence, untouched |
| `native_body_character_scope_bound.repair2_artifact.json` | `1ad2b56f653f0ea22784f9a3c975556ee7771b0f0b365ec169f7914624661638` | second repair, untouched |
| `native_body_character_scope_bound.repair2_run.evidence.md` | `6560eb028b573260f58b6dd23bc97e7df733aa880c85c9831fc492fafd9bc67c` | second-repair evidence, untouched |
| `native_body_additive_assignment.evidence.md` | `6c229c9814cccb180580b0b960c3cbcbd79e7284ba0f698f80db22767c5ef93f` | v4 evidence, untouched |
| `native_body_order_lineage.{json,evidence.md,addendum.md}` | `3b08509c…` / `891422ed…` / `1f903250…` | untouched |
| `native_body_uncertainty_split.{json,evidence.md,addendum.md}` | `478054d5…` / `5a062aad…` / `efb67ba3…` | untouched |
| `native_body_character_shadow.json` | `55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361` | untouched |
| `data/parse/layout_bakeoff.yaml` | `530ce05297e881ac91b7404c608dec5aadd9ffd22b7bbbbdd68e064b534290b5` | declaration pin, SAME |
| `crates/da-parse/tests/fixtures/preserved/provenance.yaml` | `028a5d62cc8412097ea03ee5e529bae67bf8f9d4fe7819116e6365ad2aca0ebf` | oracle pin, SAME |
| hub / rlm / reca TEI | `98546123…` / `e6aa41f5…` / `dac18882…` | oracle bytes, SAME |
| hub / rlm / reca PDF | `ab3a5139…` / `8567362c…` / `a5672497…` | pinned inputs, SAME |

The rejected **v4 worktree source bytes are not recoverable**: the auto-commit
was aborted, the index holds only an earlier 559-line partial snapshot (preserved
at `s05_t01_body_boundary_grounding_run/preserved_before/native_body_text.rs`, sha
`49c286b0e3ea46c6e4735d55962a4b5ba1366308427608fb29c24c5e9785d992`) that does
not contain `build_native_body_assignment`, and the v4 measurement itself is
preserved as
`preserved_before/native_body_character_scope_bound.v4.json`
(sha `14f76bba…`). The v4 source was rejected by review and is superseded by
this repair; its recorded per-file sha256 values appear in the additive evidence.
This limitation is named rather than papered over.

## 3. Run (exact metadata; original exit codes; no execution timeout)

Capture script sha256 `b0a68bba45753e26a18b3b74c062d2e19f229153d813ba4b6bdfa6e9902e4eb9`
(identical at start and after the run, `RUN.identity.txt`). Run window (UTC)
`2026-10-01T00:58:40.208128764Z` → `2026-10-01T01:00:22.143032843Z` (~102 s;
the build cache was warm).
`git_head = a6facbef77a7a53574883dd451270fa87d2891e0`, `cargo 1.98.1`,
`rustc 1.98.1`.

| step | command | elapsed ms | rc | stdout B | stderr B |
|---|---|---|---|---|---|
| 01 | `cargo test -p da-parse --test native_body_character_oracle` | 3522 | 0 | 3288 | 257 |
| 02 | `cargo test -p da-parse --test native_body_uncertainty_split_contract` | 407 | 0 | 811 | 210 |
| 03 | `cargo test -p da-parse --test architecture_contract` | 877 | 0 | 7600 | 176 |
| 04 | `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` | 1234 | 0 | 0 | 139 |
| 05 | `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` | 870 | 0 | 0 | 15091 |
| 06 | `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` | 826 | 0 | 0 | 15091 |
| 07 | `cargo run -p da-cli --features pdf-layout --example native_body_character_probe` | 64561 | 0 | 1859 | 15054 |
| 08 | `cargo fmt --all -- --check` (LAST Rust action) | 29353 | 0 | 0 | 0 |

Test totals from the captured stdout: **47 passed** (oracle), **11 passed**
(uncertainty split), **101 passed** (architecture). The ~15 091 B stderr on the
clippy steps is vendored `samyama-graph` warning noise outside this change; a
scan of those captures for a warning naming `da-parse` or `da-cli` returns 0
matches, and `-D warnings` returned rc 0 on the changed crates. Source shas after
the final run: `native_body_text.rs` `be375e3b…`, `lib.rs` `c62cd8f2…`, oracle
test `4d8cae03…`, probe `fe618385…`, `Cargo.toml` `58850622…`; the uncertainty
split test `34eb794d…` and `architecture_contract.rs` `6e951a20…` are unchanged.

## 4. Measured result (the real three pinned PDFs)

| document | segmentation Body tokens | heading Body tokens | bounds (seg / heading) | verification | adopted | body gap |
|---|---|---|---|---|---|---|
| hub-2507.19457 | 0 | 40 390 | `None` / `(0,1)`–`(95,55)` | `single_route_uncorroborated` | none (refused) | not_assessable |
| rlm-2512.24601 | 4 681 | 5 934 | `(0,40)`–`(42,5)` / `(0,1)`–`(9,4)` | `conservative_extent_intersection` | intersection `(0,40)`–`(9,4)` | not_assessable |
| reca-2605.26525v1 | 0 | 6 366 | `None` / `(0,1)`–`(9,50)` | `single_route_uncorroborated` | none (refused) | not_assessable |

Additive aggregate: 3 declared / 1 contributing (rlm) / 2 typed refusals
(`SingleRouteUncorroborated`), population incomplete. Folded P1 axis
`not_assessable` / `P1LocalizationUncertain { 15 322 in-token, 6 spanning, 2 material }`;
folded P2 axis `not_assessable` / `P2LocalizationUncertain { 13 384 in-token,
214 spanning, 1 766 material }`; the occurrence-conserving
`in_plane_not_in_body = 0` (no proven P2 body deficit). `d498_reopen_required =
false`, `thresholds = not_accepted`.

## 5. What this proves, and what stays unresolved (no clearance)

**Repaired and proven.** The false whole-run agreement is gone: hub and reca no
longer adopt a body from a single route whose boundary token the marker route
labels `Header`; rlm's adopted body is the conservative intersection of the two
routes, not the longer heading run. The join policy is checked, the P1/P2 axes
are both published, and the P2 deficit is occurrence-conserving. The four
negatives the review named are tested by name (single-route refusal demonstrating
the old `Header` hole, shared-start/different-end intersection, scope join
mismatch, both axes published).

**Still unresolved, named (no invented clearance).**

1. **P1 is `NotAssessable` for all three documents.** `absent_from_plane = 0`
   everywhere, with 69 400 in-token + 92 spanning + 17 material uncertain
   occurrences at corpus level. `GranularityOnly` is correctly withheld (a
   zero-absent plane is not preservation), there is **no proven content loss**,
   so `d498_reopen_required = false` and no D498 decision is recorded. The
   body-gap question is **not** closed.
2. **The additive population is 1/3.** hub and reca have no marker `Body` run at
   all, so no independent corroboration is possible from the declared routes;
   the honest outcome is a named refusal, not a single-route waiver. A future
   engineering fix would need a genuinely independent second native entry for
   those documents (for example a second segmenter whose rule is not the same
   heading vocabulary) — deliberately not invented here.
3. **rlm's intersection ends at page 9** because the heading route's `Body` run
   ends there while the marker route continues to page 42. The adopted extent is
   therefore conservative; the remainder is not claimed. Its P2 axis is
   uncertainty (unbound occurrences), not a proven deficit.
4. Locks unchanged: `claim_scope = diagnostic_only`,
   `product_coverage_effect = none`, `promotion_state = candidate`,
   `auto_promote = forbidden`, `graph_mutation = forbidden`,
   `import_eligible = false`; `thresholds = not_accepted`.

## 6. Negative / boundary cases covered by the tests in this run

* a single-route boundary whose S03 token is `Header` is refused
  (`SingleRouteUncorroborated`, no body) — the old hole;
* a shared start with a different end is never whole-run agreement
  (`ConservativeExtentIntersection` with `adopted_bounds` = the intersection);
* different starts adopt only the intersection both routes carry;
* the same complete first-and-last extent is the only whole-run agreement;
* a scope that names a join policy other than the body's is refused
  (`ScopeJoinMismatch`);
* an occurrence absent from the plane is not re-counted as a P2 body deficit
  (`unresolved_from_plane = 1`, `unresolved_from_body = 2`,
  `in_plane_not_in_body = 1` in the same document) — occurrence-deficit
  conservation;
* both axes are published when P1 is uncertain and P2 is a region gap;
* pre-existing axis-1 negatives remain green: equal-length different content,
  duplicate/surplus masking a selective loss, reordering, arbitrary substring in
  token, repeated occurrences not excused by one candidate, rung/case mismatch
  refusals, greedy-anchor conservatism, empty oracle, same-generator oracle.

## 7. Runs preserved separately (immutable, not reused)

| directory | state |
|---|---|
| `s05_t01_body_boundary_grounding_run/` | intermediate run of the pre-final source (46 oracle tests); all eight steps rc 0; preserved byte-for-byte, not the producing run |
| `s05_t01_body_boundary_grounding_run2/` | producing run: 47 oracle tests, all eight steps rc 0, `SAME source/input/preserved`, `SAME artifact` |

No failed run exists for this repair: both runs completed with all eight steps
rc 0, and neither directory is written by any other run.
