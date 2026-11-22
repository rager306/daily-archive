# M520 S01 — native PDF candidate: resource and first-PDF evidence audit

Date: 2026-10-16. Milestone `M520-zxj5e9`, slice `S01`.
Task **T01** authored the audit; task **T02** corrected the admission boundary
and recorded the source-contract disposition (§11–§12). Kind: **metadata-only
read-only research audit.** Not a GSD task acceptance, not product or scientific
acceptance, not a rights approval.

Manifest: `artifacts/parse_compat/native_candidate_m520/resource_audit.json`
Governance contract: `scripts/tests/test_m520_resource_audit.py`
Source contract (T02): `artifacts/parse_compat/native_candidate_m520/source_contract.json`

## 1. What this audit does and does not establish

Separated claims, because collapsing them is how a blocked resource becomes a
false "ready":

| Claim | Established here? |
|---|---|
| Metadata inventory of 3 weights + 1 gazetteer under the local GROBID root | yes, partial and explicit |
| Byte identity of those 4 files + the one source PDF | yes (size + SHA-256) |
| A *complete* resource inventory or a genuine export | **no** — other channels are unknown |
| That any asset may be exported, copied, redistributed or used natively | **no** — rights stay blocked |
| That the native candidate produces a correct scholarly result | **no** — all 3 diagnostics remain negatives |
| That the selected PDF is a positive fixture | **no** — two short refs do not make a bibliography |
| That the visual page layout was reviewed | **no** — no page image was rendered or inspected |

This is an explicitly **partial** inventory of three CRF weights and one
gazetteer. It is not all resources and not a genuine export.

## 2. Authority and bounds

- **D127** — `import_eligible=false`; graph write and promotion stay locked.
- **ADR-076 §14** — `deployed=false`, `auto_promote=forbidden`,
  `graph_write=forbidden`.
- **D449** scopes **M512**. It is not a blanket resource permission.
- **D491** removes C3 / independent-rights-review as a *prerequisite for hobby
  research*, so this metadata-only audit needs no fresh per-asset legal census.
  It does not grant export, native-use or promotion permission.
- `data/parse/grobid_model_rights.yaml` stays **byte-identical**
  (SHA-256 `e0c77b4b92eb717fa82b951dd46f0d33f0ad0c7311f88c723673c3babe05ef2f`):
  `external_service_only`, `redistributable: false`, `rights_status: blocked`,
  `missing_or_ambiguous_rights: fail_closed`. No export/native-use/promotion
  permission is inferred.

The authority block is closed to exactly `D127`, `D449`, `D491`, `ADR-076`: an
added, duplicated or reordered reference is rejected at admission.

Method: bounded, directly enumerated reads only — **no recursive scans**. No
cargo, no build, no fmt, no network, no inference, no resource export, no
graph/import/deploy mutation, no cleanup. Only the four declared files were
written.

## 3. Trusted roots and the protected map

Paths are only ever named as **aliases** in the manifest; the alias→host-root
mapping lives in the governance test, so a manifest can never select a read
root. Unreachable roots degrade to `unavailable`, never to a fetch.

| alias | role |
|---|---|
| `repo` | this repository |
| `grobid-home` | trusted local GROBID resource root |
| `rust-grobid-useful-pdf` | trusted external evidence root (protected map, review) |
| `rust-grobid-integration` | trusted external evidence root (diagnostics, plan) |

Existing protected source pins are preserved through the trusted
`authorized-cli-clean-before-bindings.json` **current25** map:

- alias `rust-grobid-useful-pdf`, `authorized-cli-clean-before-bindings.json`
- size 5724, SHA-256 `e1d61c9ae05f8c4e998e3a21e4104135a9bb7b42236fffffd8b252ea3f5abe10`
- 25 entries; map is not replaced or trimmed, and its contents are not committed.

The contract re-hashes all 25 pinned repo files whenever the map is reachable. A
drifted pin fails the contract.

## 4. Resource inventory (exact pins)

Root `grobid-home`; sizes and SHA-256 confirmed by direct re-measurement.

| id | relative path | kind | format | stage | bytes | SHA-256 |
|---|---|---|---|---|---:|---|
| `grobid_model_header` | `models/header/model.wapiti` | crf_model_weights | wapiti | header | 24034608 | `9966a973faa662e0c966774c64586aff07e64d96bde520bb7252a656219e491a` |
| `grobid_model_reference_segmenter` | `models/reference-segmenter/model.wapiti` | crf_model_weights | wapiti | reference-segmenter | 12167531 | `cf51216fc62acbfd0dacff568c5f9bc99a9689e3c6e01af0307b939bf4d1fe8c` |
| `grobid_model_citation` | `models/citation/model.wapiti` | crf_model_weights | wapiti | citation | 22103761 | `5f17697840c78fa9595119aa7f579f5e9576fbc879477c47507a5f9a6bc264f5` |
| `grobid_lexicon_places_location` | `lexicon/places/location.txt` | gazetteer_lexicon | plain_text_lines | location | 10302767 | `74cd4d6bbf38643a6f1dcfbf342c45a7a786cd10bd9db817fa37b0fa65cf51c8` |

All four are regular files (no symlink). `kind`, `format` and `stage` are bound
to the *identity* of each resource (the `(kind, format, stage)` tuple is checked
per id), not merely admitted as strings from some allowed enumeration: the
schema closes `format`/`stage`/`kind` to the declared vocabulary, and the
binding check then requires each resource to carry the one value pinned to its
own id. No raw model bytes, no lexicon entries and no credential content are
committed — the manifest carries identity, numeric counts and closed
dispositions only.

The lexicon's `counts` block is the only numeric observation in the manifest and
is pinned as well (`raw_lines=620618`, `unique_trimmed_nonblank_lines=535792`,
`blank_lines=0`, `whitespace_only_lines=0`), re-derived independently from the
same byte-pinned file. A changed count, or a `counts` block on any other
resource, is rejected as an identity mismatch.

### Lexicon line semantics (kept separate)

`lexicon/places/location.txt`:

- **raw lines = 620 618**, basis `newline_terminated_physical_lines` (the
  trailing-newline count equals the physical-line count).
- **blank lines = 0**, **whitespace-only lines = 0**.
- **unique trimmed non-blank lines = 535 792**, normalization
  `trim_ascii_whitespace_then_drop_empty`, dedup `byte_exact_after_trim`.
- **current loader applicability = `exceeds_current_caps`.**

620 618 raw lines and 535 792 unique trimmed non-blank lines are recorded as
**two separate observations**. Raw-line count is not put forward as a usable
entry count, and a byte-exact-after-trim dedup is not put forward as a general
normalization contract.

### Caps stay unchanged

| cap | value | source |
|---|---:|---|
| bundle entries per channel | 100 000 | `MAX_LEXICON_ENTRIES_PER_CHANNEL` |
| gazetteer entries per lane | 65 536 | `GAZETTEER_MAX_ENTRIES_PER_LANE` |
| serialized loader bytes | 16 777 216 | `MAX_LEXICON_BYTES` (`lexicon_loader.rs`) |

Location's 535 792 unique entries exceed both cardinality limits, so an
"export everything" bundle is **technically incompatible** with the current
limits. No cap is raised, and no subset, sharding, name alias or cherry-picked
target-PDF-only channel is produced.

## 5. First-PDF evidence (independent source text)

Source: `data/sources/arxiv/math-oc/1203.2295/source/1203.2295.pdf`
(383 369 bytes, SHA-256
`f4594edaccccad936def8b1b4673d6d4796f624e9952f1e94a650cfc3ea94608`). The file is
**git-ignored** (`data/**/*.pdf`), so a clone without local bytes must report it
**unavailable** — never as pass (§9), and never by silent download.

Independent extraction (Poppler `pdftotext`, text only — **no page image was
rendered or visually inspected**):

```
pdftotext <existing.pdf> -                      # plain
pdftotext -layout -f <page> -l <page> <existing.pdf> -
```

Physical and printed page numbers coincide for the bound pages (physical page 1
prints "1", page 2 prints "2", page 4 prints "4", page 10 prints "10", page 11
prints "11"); page count 11.

| what | exact printed text (observation) | chars | physical / printed page |
|---|---|---:|---|
| title | `TECHNIQUES FOR SOLVING SUDOKU PUZZLES` | 37 | 1 / 1 |
| reference [11] | `[11] K. Lange, Optimization, Springer-Verlag, New York, 2004.` | 61 | 10 / 10 |
| reference [18] | `[18] S. S. Skiena, The Algorithm Design Manual, Springer-Verlag, 2nd ed., 2008.` | 79 | 11 / 11 |
| in-text callout → [11] | `[11]. We will denote the projection operator taking y to x by PC (y) = x.` | 73 | 4 / 4 |
| in-text callout → [18] | `it becomes a full solution or violates a constraint [18]. In the latter case it backtracks` | 90 | 2 / 2 |

The declared snippet bound is **90 characters**, taken from the longest actual
snippet above — not from a new product limit. Snippet fields are the only
non-closed content admitted, and they are exact pinned literals: each must equal
its recorded observation verbatim, stay inside the measured bound, and stay
marked `independent_review=false`. The manifest states
`semantic_truth_asserted=false` and `substring_scan_is_proof=false`. A nonempty
snippet is not semantic proof and substring presence is not used as proof.

**Honest scope limits on the bibliography.** The reference list carries **22**
numbered labels; only **[11] and [18]** are transcribed here. All 22 remain
wrapped/incomplete at the physical-line level and are **unresolved**. Two
selected references **do not** establish a complete bibliography, and callout
linking is **not** established. `has_complete_labelled_line_inventory()` and the
marker linker correctly refuse linking against an incomplete inventory.

## 6. Three diagnostic negatives (preserved identities)

All three candidate diagnostics remain **negatives**. Root
`rust-grobid-integration`; report and TEI identities are recorded by size and
SHA-256 and re-verified when reachable.

The contract compares the **entire** diagnostic mapping against an independent
literal table (three distinct document ids, each trusted alias, report path,
report size, report SHA-256, TEI path, TEI size, TEI SHA-256, outcome and
outcome code). A repeated id, an omitted id, a substituted or cross-swapped
path/size/hash, an extra diagnostic or a malformed entry is rejected before any
file is opened — even when the local bytes happen to be unreachable. Missing
diagnostic bytes are reported **separately** as `unavailable`; they are never
identity success and never positive success.

| document | report.json (bytes / SHA-256) | candidate.tei.xml (bytes / SHA-256) | outcome |
|---|---|---|---|
| `1203.2295` | 3717981 / `b91f09886c37eacddea8d50287b510d9a4910ad78b35c705676638d4057c29c5` | 14588 / `25ee98d6590234d336e7c22ef4b59bb7fddf47923ebb3a82bb2c3efe1cbddfbc` | header refuses a zero font size; reference stage processed 73 physical lines and admitted zero records; body empty; callouts unlinked; visual Unsupported |
| `1506.02438` | 6439357 / `1680968c32c379ca8f90366f2c963a8ad417e7acb7c089a8d1ba20001d73dd5f` | 45012 / `485d0ce54c4748e9e70e88d254f2b1d3d8859a87125b554a2e582701433ae674` | header refuses `font_order=9.961999`; references `RegionAbsent`; visual heads `Terminal: unsupported` |
| `1409.0473` | 6564296 / `c945ceca6ceb64035b01012e512d385607e8bda551cccaa07760a3ccf60ca3e9` | 51380 / `0126507ae4e9f21b242fd06940fcbe977bcfe000cfe51824610db9f0d36809ec` | same refusal shape as `1506.02438` |

### 6.1 Exact source reasons (read from the preserved reports)

The typed refusal the existing reports actually carry, re-read from those bytes
(no new parser run, no report edited, no outcome code repinned):

- **`1203.2295`** — `header.status = refused` with
  `HeaderInput(InvalidFontSize { page: 6, source: 214, milli_pt: 0 })`;
  `references.status = executed_candidate`, `records = 0`,
  `complete_within_one_labelled_physical_line_envelope = false`, and
  `ReferenceInventoryIncomplete`.
- **`1506.02438` and `1409.0473`** — `header.status = refused` with
  `HeaderFeatures(InvalidStatistic { name: "font_order", value: 9.961999 })`;
  `references.status = refused` with
  `RegionAbsent { region: References, requested: true }`.

**Label caveat, recorded so the label is not read as evidence.** The historical
outcome-code spelling
`header_font_mean_refusal_and_references_region_absent` is a *label*, not proof
that the rejected statistic was named `font_mean`. The reports reject
`font_order` at `9.961999`, one f32 ULP below the minimum. The old reports were
not modified and the old codes were not silently repinned.

A process exit 0 does not imply header/reference/citation success. The empty
`1203` citation-feature digest is the SHA-256 of empty bytes and does not prove
citation-model inference.

**Synthetic diagnostic.** `synthetic-diagnostic-lexicon.yaml` (220 bytes,
SHA-256 `88aa7e02047ee0c8492bef723e71c10ae4061d32f4057511b8fe6943a0e69209`) is
recorded as **synthetic** with `scholarly_lexicon=false`,
`copied_to_carrier=false` and `usable_as_real_resource=false`. It is not copied
into the new carrier directory and is not a scholarly lexicon.

**Reference plan.** `/tmp/rust-grobid-integration/gsd-first-pdf-resource-plan.md`
(SHA-256 `4531a05f1e8f6f13b70274032ce9f35a668a53402dcdae028d42692f4f2873cd`) was
consulted as **leads only**; it has no authority over D491 or this manifest.

## 7. Named readiness blockers (closed codes)

Each blocker is a closed code with a closed effect — no free-form quoted
resource string. The *set* of blocker codes must equal the declared closed
vocabulary exactly, so a blocker cannot be dropped to fake readiness.

| code | resource | effect |
|---|---|---|
| `lexicon_export_rights_blocked` | `grobid_lexicon_places_location` | resource_not_ready |
| `gazetteer_exceeds_lane_cap` | `grobid_lexicon_places_location` | resource_not_ready |
| `bundle_cap_exceeded` | `grobid_lexicon_places_location` | resource_not_ready |
| `no_lexicon_exporter_available` | `grobid_lexicon_places_location` | capability_absent |
| `source_version_drift_0_9_1_vs_0_9_0` | `grobid_model_header` | version_drift |
| `java_rust_feature_abi_discrepancy` | `grobid_model_header` | abi_mismatch |
| `font_mean_one_ulp_defect` | `pdf_arx_1203_2295` | quality_unverified |
| `zero_font_annotation_contamination` | `pdf_arx_1203_2295` | quality_unverified |
| `region_assignment_no_body_transition` | `pdf_arx_1203_2295` | quality_unverified |
| `reference_inventory_incomplete` | `pdf_arx_1203_2295` | positive_fixture_absent |
| `visual_layout_not_independently_reviewed` | `pdf_arx_1203_2295` | quality_unverified |
| `no_independent_positive_fixture` | `pdf_arx_1203_2295` | positive_fixture_absent |
| `other_channels_unknown_unready` | `grobid_lexicon_places_location` | resource_not_ready |

Version and ABI remain **explicit blockers, not silent repins**: the local
source tree reports `version=0.9.1` (`gradle.properties:1`) while the pinned
baseline is `0.9.0-crf`. Two concrete feature-construction gaps (font-identity
vs font-size semantics; reference-segmenter character-length vs token-count
position units) are not fixed by adding lexical resources.

## 8. Readiness verdict

```
first_pdf_source_bound          = true   # historical audit claim, needs current bytes
positive_fixture_ready          = false
complete_bibliography_established = false
callouts_linked_established     = false
lexicon_export_ready            = false
native_access_ready             = false
visual_layout_reviewed          = false
other_channels                  = unknown_not_ready
rights_approval_claimed         = false
verdict                         = blocked_pending_resources_and_positive_fixture
```

The first PDF is **source-bound** (title, two printed references and their
in-text targets are independently recorded). It is **not** a positive fixture:
a usable, legally cleared native lexicon export remains blocked, no
independent positive fixture exists, and no scholarly quality or rights
approval is claimed. `readiness.first_pdf_source_bound` is marked
`source_bound_requires_current_byte_corroboration=true`: the claim is accepted
only when the current check on the pinned PDF bytes says `match`.

## 9. Governance contract — one fail-closed admission boundary

`scripts/tests/test_m520_resource_audit.py` runs a **single admission entry**
(`admit_and_verify`) with two phases. The entry is the only path by which a
manifest-declared file can be read.

**Phase 1 — structural admission (pure; no filesystem access).** Closed schema
and key+type vocabulary; every manifest-declared path checked for
absolute/traversal/backslash/credential shapes; exact decision-reference list;
per-id resource identity binding (kind/format/stage included); the entire
three-diagnostic mapping compared with an independently fixed literal table;
exact pins for the protected map, reference plan, synthetic diagnostic and the
independent review; snippet literals compared with the recorded observations;
`data/parse` names and byte pins; rights-source digest; unchanged caps; and the
closed blocker set with a still-blocked readiness verdict. **Any failure returns
immediately** — nothing on disk is touched, so a rejected manifest cannot reach
an open or a hash.

**Phase 2 — current availability (filesystem).** Only the independent literal
descriptors drive this phase, never re-read manifest entries. Containment is
decided by `realpath` *before* any read, so traversal and symlink escapes are
rejected without opening the target; a symlinked identity is refused.

**Two separated axes.**

| axis | where it lives | what it means |
|---|---|---|
| historical observation | manifest `historical_observed_state` | what the audit saw on its own host |
| current availability | computed at verification time | what *this* checkout actually has |

`historical_observed_state` may never imply current verification: the report
derives `current_is_present` only from the current check, and
`current_source_bound` only from the current PDF status. `current availability`
is never written back into the manifest
(`current_availability_stored_in_manifest=false`).

**Typed report.** `admit_and_verify` returns per-resource and per-diagnostic
`current_status` (`match` / `mismatch` / `unavailable`), a `required_current_status`
map, `declared_source_bound` vs `current_source_bound`, closed
`rejection_codes`, `consistency_codes`, and `open_targets` (every path handed to
the content reader). Verdict is one of `rejected_fail_closed`,
`admitted_unverified`, `admitted_verified`. `admitted_verified` requires every
required byte to be `match` and no consistency code — so a missing
git-ignored PDF, an absent protected map or unreachable diagnostic bytes yield a
typed `unavailable`/`mismatch` verdict, never a silent pass.

The committed manifest is exercised through this same entry by
`test_real_manifest_admits_and_never_claims_verification_it_cannot_show`, which
asserts the biconditional — an `admitted_verified` verdict exactly when every
required byte matched, with a reason code for each non-matching one — instead of
requiring this host to hold the git-ignored PDF, the vendor models and the `/tmp`
evidence. On a bare clone the same entry returns `unavailable` and
`admitted_unverified`, the test still holds, and the tracked fact is that no host
can reach `admitted_verified` without the bytes.

**Negative controls** (inert bytes/paths in a temp directory only): missing PDF
and absent protected map; unknown/extra/reordered decision reference;
identity-inconsistent `format`/`stage`; substituted resource identity;
duplicated / omitted / cross-swapped / extra diagnostics; credential-like,
absolute, traversal and backslash paths; in-root and escaping symlinks; and
wholesale rejection of an incomplete or malformed manifest. Each rejection is
paired with an **opener spy** asserting **zero** target opens, so the tests
prove the boundary, not merely that a separate predicate returned an error.

**The source-contract check entry** (`check_source_contract`) validates the T02
disposition against the same pinned identities and the consumed audit, and
rejects fake permitted/ready states.

Run:

```
PYTHONPATH=scripts python3 -m pytest scripts/tests/test_m520_resource_audit.py -q
git diff --check
```

## 10. Assumptions and bounds reported honestly

- `historical_observed_state` is an observation on the audit host. Current
  availability is recomputed at verification time; when a resource is
  unreachable in another clone the current status is `unavailable`, the readiness
  claim is not corroborated, and nothing is acquired.
- The source PDF is git-ignored, so its absence in a clone is expected and must
  be reported. It is not a pass, and it is not repaired by a download.
- Snippets were produced by a specific Poppler extraction and are recorded with
  their mode; they are observations, not reviewed truth.
- The inventory is partial by construction; channel completeness is not claimed.
- Host absolute paths, raw model/lexicon content and credentials are
  deliberately absent from the committed JSON.

## 11. T02 corrective revision (provenance)

T01's output was independently reviewed at
`/tmp/rust-grobid-useful-pdf/native-t01-output-independent-review.md`
(6435 bytes, SHA-256
`1ab7b8fc3d7a6284f3c3747b9d75b365981db34578d4aa6eb754f37b38df6f14`), which
found four blockers. The manifest pins those review bytes so the corrective
scope cannot be silently re-pointed at a different document.

| finding | correction |
|---|---|
| availability not integrated; unavailable accepted as a local return value while the manifest still claimed present/source-bound | Phase 2 current availability + `current_source_bound` + `declared_source_bound_not_corroborated`; history separated from current bytes |
| non-snippet content vocabulary open (`format`, `stage`, `decision_refs`) | per-id `(kind, format, stage)` binding **and** closed `format`/`stage`/`kind` enums in the schema; decision refs closed to `D127, D449, D491, ADR-076`; the lexicon `counts` block pinned so no other non-snippet field admits free input |
| three diagnostic identities self-declared, not independently exact-pinned | independent literal diagnostic table with whole-mapping comparison and substitution/duplicate negative controls |
| validation failures did not prevent later asset opens | one fail-closed pre-open admission boundary; filesystem checks consume only admitted pinned descriptors; opener spy asserts zero opens |

The T01 source revisions named in the review are superseded by this T02 revision
of the same three paths. T01 history is **not** rewritten, relabelled or
replayed (`rewrites_t01_history=false`); the audit facts and the blocked
disposition are unchanged.

## 12. T02 source-contract disposition

`artifacts/parse_compat/native_candidate_m520/source_contract.json` consumes the
corrected audit and records a **typed blocked disposition** under a closed
schema:

- `disposition = blocked`, `positive_fixture_ready = false`,
  `expected_positive = null`, and `no_pretend_success` keeps
  `expected_title`, `expected_references` and `expected_callouts` null with
  `scientific_quality_claimed = false`.
- Identity carries the exact source-PDF identity and the five resource
  identities; `blocker_codes` equals the audit's closed blocker set; policy
  locks and the rights boundary stay blocked.
- `blocked_dimensions` names each blocked axis (resource readiness, lexicon
  export/use, native access and ABI, independent positive fixture, visual
  review, bibliography and linking, scholarly quality, rights).
- `required_to_unblock` names what must be evidenced before any positive branch
  (lawful/interface-compatible native access, cleared lexicon export or use
  rights, publicly reviewed cap policy change, version and feature-ABI
  alignment, independent visual layout review, complete labelled bibliography
  inventory with callout linking, Java renderer comparator rows under a separate
  identity).
- `next_action = stop_and_public_replan_before_any_source_work`.

Separately from the disposition, unchanged facts: the audit is not a positive
fixture, the three diagnostics stay negative, and no positive claim is made.
The source PDF identity here is the *historical* audit identity; the T02
disposition does not restate the current availability of the bytes.
