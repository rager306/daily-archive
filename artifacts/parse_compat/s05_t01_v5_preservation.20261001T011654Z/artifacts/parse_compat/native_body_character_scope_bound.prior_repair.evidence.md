# S05/T01 native body-character repair — run evidence

Milestone: `M514-1lss2s` · Slice: `S05` · Task: `T01`
Finding repaired: `S05-T01-COUNTS-DO-NOT-PROVE-PRESERVATION` (brief `S05-T01-SCOPE-BOUND-PRESERVATION`)
Producing run: detached `cargo run -p da-cli --features pdf-layout --example native_body_character_probe`
Report: `artifacts/parse_compat/native_body_character_scope_bound.json`

## What the repair changed

The previous diagnostic issued a verdict from a **length ratio** against an unrecorded
980-per-mille floor, labelled the HUB surplus `granularity_only`, and folded the S03
population from the single contributing document only. The repair replaces the verdict
with a scope-bound **lexical unit location** over the real characters:

- every side declares `BodyScope` (side, included classes, join policy, normalization);
- units are tokenized **per fragment**, so concatenating two tokens cannot merge a word
  boundary and fabricate loss;
- each oracle unit is located as **exact**, **boundary variant** (one token merged it as
  prefix/suffix, or the oracle merged consecutive tokens inside one fragment), or
  **unresolved**;
- `ContentLoss` requires at least one **unresolved** unit in the whole native plane;
- a unit resolved in the plane but unresolved in the body is a typed
  `P2RegionAssignmentGap` (`NotAssessable`), never relabelled as loss;
- the aggregate carries the complete declared population and names every absent document;
- the oracle TEI bytes and the input PDF bytes are sha256-verified from bytes against
  `crates/da-parse/tests/fixtures/preserved/provenance.yaml` and
  `data/parse/layout_bakeoff.yaml` before any comparison.

The historical `artifacts/parse_compat/native_body_character_shadow.json` is not
overwritten; the repaired diagnostic is the separate artifact above.

## Run (original exit code preserved)

| field | value |
|---|---|
| probe rc | `0` |
| start UTC | 2026-09-30T18:57:02Z |
| end UTC | 2026-09-30T18:57:57Z |
| stdout | `/tmp/t01_probe_final2.stdout` (1236 bytes, 4 lines) |
| stderr | `/tmp/t01_probe_final2.stderr` (0 bytes) |

## Hash bindings (before == after; captured before the producing run)

| path | sha256 |
|---|---|
| crates/da-parse/src/native_body_text.rs | `e0c37d132d9d6b1ad141f8e89c8869ec2c03a0e1153ef4791d214ee9c26336c6` |
| crates/da-parse/src/lib.rs | `f5728f087462f1f6b8869b6ff454cbacce628288beb3bc26ef5657ad35c222f4` |
| crates/da-parse/tests/native_body_character_oracle.rs | `0414e80ed5328e701f8708094189f13f55a09f99dfd4aaeb2b3a9538be4d676b` |
| crates/da-parse/tests/architecture_contract.rs | `a0bd3864d5bf0aa7ad5da5aa10d52a62cf60c6fa8746861ba5acd94f22e0910c` |
| crates/da-cli/examples/native_body_character_probe.rs | `77546ae0c52717bdce8acb00a55888f5060450fe309f3d4bfd1cf3b075638f3e` |
| crates/da-cli/Cargo.toml | `6e9e213e7e1e6b13d65f7f3a9a69d48b346c20be0677e5bdb37895e42bca5eb8` |
| data/parse/layout_bakeoff.yaml | `530ce05297e881ac91b7404c608dec5aadd9ffd22b7bbbbdd68e064b534290b5` |
| crates/da-parse/tests/fixtures/preserved/provenance.yaml | `028a5d62cc8412097ea03ee5e529bae67bf8f9d4fe7819116e6365ad2aca0ebf` |
| hub TEI | `985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4` |
| rlm TEI | `e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3` |
| reca TEI | `dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040` |
| hub PDF | `ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec` |
| rlm PDF | `8567362c22768d9b50d4a4a8d63bb28dda2c2b2051be30d67f70f645170429ca` |
| reca PDF | `a5672497671518d3b3d79698becfd5a4bbdc512bb02a002a473db501c3b097c8` |
| historical shadow artifact | `55647743d0a75fdea4e21222f03c3a66bdd6529b436d55db58bf4e1bc7ee4361` |
| repaired scope-bound artifact | `ce87e3af6fc9f332520df77463001746aecd76fc4a82d949b1974b28d2ae9d3f` |

Oracle TEI pins verified from bytes for all three documents (probe would exit non-zero
otherwise). PDF pins verified from bytes.

## Measured result (P1 = all-native plane vs oracle; per unmatched-unit location)

| document | oracle units | exact in plane | boundary variant | unresolved (P1) |
|---|---|---|---|---|
| hub-2507.19457 | 33 094 | 32 914 | 81 | **99** |
| rlm-2512.24601 | 15 713 | 15 700 | 8 | **5** |
| reca-2605.26525v1 | 21 516 | 21 378 | 118 | **20** |
| corpus (P1) | 70 323 | 69 992 | 207 | **124** |

S03 parent-span band: `hub` and `reca` are typed `NoBodyTokens` (absent); `rlm` measured
`ContentLoss` with 5 unresolved units. Corpus aggregate: declared 3, contributing 1,
absent 2, `population_complete=false` — the S03 band does not mask the two absent
documents (it reports them by name). Word/heading band: all 3 contributing,
`ContentLoss` at 5–99 unresolved units.

## Remaining blocker (honest)

Matched-scope **P1 content absence remains**: 124 unresolved oracle units corpus-wide
(0.18 % of 70 323) are absent from the whole native plane in every located form. Because
the plane is a scope superset of the oracle, those absences are matched-scope P1 loss
evidence; `ContentLoss` and `d498_reopen_required=true` stand for all three documents and
for both bands. The blocker is **not** waived or relabelled.

The residual boundary-variant units (207 corpus-wide, e.g. merged compound words) are
located as tokenization granularity, but they are reported as evidence only and do not by
themselves clear the unresolved residual. Whether the 124 unresolved units are genuine
character loss or a normalization/encoding difference (math symbols, diacritics) is not
resolved by this measurement and is stated as such rather than assumed.

## Verification performed

- `cargo test -p da-parse --test native_body_character_oracle` → 17 passed
- `cargo test -p da-parse --test architecture_contract` → 100 passed
- `cargo clippy -p da-parse --no-deps -- -D warnings` → rc 0
- `cargo clippy -p da-parse --lib --tests --no-deps -- -D warnings` → rc 0
- `cargo clippy -p da-cli --no-deps -- -D warnings` → rc 0
- `cargo clippy -p da-cli --lib --tests --no-deps -- -D warnings` → rc 0
- `cargo clippy -p da-cli --features pdf-layout --example native_body_character_probe --no-deps -- -D warnings` → rc 0
- `cargo fmt -p kg-ontology -p kg-storage -p kg-embeddings -p kg-algorithms -p kg-pipeline -p da-domain -p da-ports -p da-application -p da-adapters -p da-parse -p da-cli -p da-mcp -- --check` (the `cargo-fmt` pre-commit hook) → rc 0
