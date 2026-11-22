---
title: "M519: opt-in Rust scholarly candidate integration"
date: 2026-10-15
status: architecture-pass-known-gates-open
milestone: M519-xy0hv9
---

# M519: opt-in Rust scholarly candidate integration

The new offline candidate connects existing PDF observations, native region carriers,
prepared CRF decoding, header fields, per-reference citation roles, TEI emission and
`TeiLifter`. It does not replace the production Java GROBID route.

This is implementation work following the
[2026-10-11 recommendations review](grobid-rust-recommendations-review-2026-10-11.md).
That review and frozen `artifacts/parse_compat` receipts are unchanged.

## Contract

- A borrowed `PreparedCrfModel` retains one immutable observation lookup per model.
  Header, reference and citation stage decoders keep their existing raw-model APIs.
- The candidate uses an opt-in lexical donor that excludes `Image` placeholders.
  Existing donor entry points keep their old behavior. Surviving source ordinals,
  text and digests are preserved; fallback page extents need not be identical.
- The new header candidate can preserve title, email and publication-number evidence
  without a recognized date. Whole author runs stay diagnostic notes, not invented
  individual authors. Affiliation linkage is unsupported.
- Each supported reference owns its source rows, printed label, citation role spans
  and emitted record. Only one explicitly labelled physical line is supported.
  Continuations and ambiguous material remain visible and block a complete link
  inventory. Exact source-bound bibliography heading lines are retained separately.
- Bibliography markers use printed Arabic labels, not physical record ordinals.
  Unresolved and ambiguous markers remain exact body text. The body is one supplied
  paragraph; fulltext structure is not inferred.
- The visual adapter observes supported top-level Image-XObject occurrences with
  encoded source-stream digests and page/placement geometry. It reuses existing
  `VisualObservation` and figure heads. It does not establish semantic figures,
  captions, tables, formulas or callout links. Nested Forms, clipping, rotation,
  shear and other unsupported placements refuse the entire lane.
  The pinned parser can also refuse valid syntax, including extra blank lines
  after comments; the adapter does not remove source whitespace to accept it.
- PDF parsing is for trusted local sources only. Limits reject admitted inputs and
  outputs; they do not bound lopdf/pdf-inspector peak allocation, decompression time
  or process lifetime. No hostile-input sandbox is claimed.
- A separate exact token diagnostic accepts caller-normalized line inventories and
  a required work budget. Complete results report an exact LCS witness. Incomplete
  results have no L or partial witness. `H <= L <= U` does not establish physical
  occurrence identity or change frozen body classifications.

- Existing `TeiLifter` preserves XML entity spellings in some projected strings;
  independently parsed TEI text and lifted strings are not claimed identical.
  The native marker scanner rejects punctuation attached to bare bracket tokens;
  those occurrences remain unlinked, not silently repaired.
- The heading-aware builder expects the original, unmodified in-process result of
  `assign_heading_regions`. Identity/row checks reject mismatched context; they
  are not an authenticity check for caller-mutated metadata.

All outputs remain candidate evidence: `deployed=false`, `import_eligible=false`,
`auto_promote=forbidden`, `graph_write=forbidden`. No default route or weights,
model limits, matching thresholds, frozen v5 body verdicts or shared legacy emitters
are changed.

## Operators

Both examples are explicit invocations, not backend selection defaults:

```text
cargo run --locked --offline -p da-cli --features pdf-layout --example native_candidate -- --pdf <trusted.pdf> --document-id <id> --model-manifest <pinned-models.json> --out <new-directory>
cargo run --locked --offline -p da-cli --example native_exact_alignment -- --oracle <tokens.txt> --side <tokens.txt> --max-work <work-units> --out <new-report.json>
```

The candidate manifest has schema `da-cli.native-candidate-models/v1`. It pins a
validated lexicon YAML digest and each configured model's bytes, feature schema,
column width, exact ordered labels and embedded-pattern digest. Paths are explicit
caller inputs, not effective configuration identity. The raw manifest digest and
path-free effective configuration digest are separate report fields. This binding
is not independent proof of Java renderer equivalence.

The exact matcher performs no implicit token normalization. Input acquisition,
UTF-8 decoding, shared collision-free interning, U/H calculation and report IO are
outside its declared matcher-work budget. Output files/directories must not exist.

## Verification

All 870 `da-domain` library tests and 414 `da-parse` library tests passed.
The domain run includes the new Send/Sync assertion; the parse run includes
prepared header and bibliography decoding. The ports crate check passed.
New contracts passed: exact alignment 11 and scholarly candidate 10. Emission
passed 8 contracts at the counted-summary stage, then 10 after the boundary
repair and two independent XML/refusal regressions described below.
The first emission run had two failed expectations; the repaired tests retain
independent XML semantic checks and cover the frozen entity/scanner limits.

All 14 adapter PDF/lexical contracts now passed. Earlier failures exposed a
wrong `ItemType` path, absent font metrics, an extra generated newline, and a
fixture image placed after both text lines. Each failure receipt is retained.
The fixed test still requires known text/image/text order, source ordinals [0,2],
and exact surviving text/digest identity. All-targets parse clippy also passed.
CLI inline tests now passed on the repaired source: candidate 3, exact 3.
The candidate tests include typed emission-refusal report retention without TEI
or overwriting an earlier run. The visual bridge passed 4 tests, including
same-ID/different-PDF binding refusal. Default exact and feature candidate checks
passed. Parse all-targets clippy passed again after the summary change.

Feature adapter clippy stopped at an existing ALTO parser branch with identical
arms (`if_same_then_else`); HEAD has the same block. The existing shared helper
is left unchanged. This failure remains open, not waived. Independent feature
CLI clippy passed after the exact example's let-chain correction. Both plain
runners built, the three fresh diagnostic attempts exited 0, and final scoped
format/check passed. These exits do not mean all scholarly stages succeeded;
per-stage outcomes are below. The later full architecture run passed all 23 steps;
its source binding and separate remaining failures are recorded below.

The existing governor test inventory ran separately: 13 passed, one failed.
`learning_critic_e2e` is already declared under `learning-evaluation`, but the
architecture runner list does not provision it. HEAD `75331d7727ec` has the same
manifest declaration, runner list and inventory test. The failure predates M519;
no rule was changed or waived. A later full-gate result does not cover it.

Source review, unit/contract tests, real-weight smoke, PDF emission, semantic
acceptance and resource savings are separate claims.

The local upstream header/reference/citation models are available and pinned.
No actual exported lexicon bundle was found. The first real-weight run on
`1203.2295`, using the existing synthetic adapter lexicon, refused emission:
`LimitsExceeded("limitations")`. The new emitter exceeded the existing 64-entry
limit; no limit was raised. The CLI returned before saving stage diagnostics,
so the exit alone does not prove each model loaded or executed. The CLI now
retains a typed report on emitter refusal and still returns the original error.
The new emitter now counts closed diagnostic categories with occurrence counts.
Its unchanged 64-entry cap bounds summary entries; all raw notes and byte caps
remain. This changes that new summary's admission semantics, not the frozen
emitter or uncertain-line inventory. Candidate source rows remain in memory;
the report does not persist them as replayable raw-row evidence.

The first runs on `1506.02438` and `1409.0473` exited 0 and saved TEI, but both
headers refused `font_order=9.961999`, references returned `RegionAbsent`, and
visual heads returned `Terminal: unsupported`. These are not successful header,
reference or figure extractions. Source review traced the header refusal to the
existing font-statistics helper: eight equal sizes yield a mean one binary32 ULP
below the minimum. The renderer correctly refuses before CRF decoding. That shared
helper remains unchanged; no candidate-only clamp or model-policy change is added.

An independent XML check matched the 8,728 and 8,906 emitted Body word hashes to
the ordered carrier digests. It found one plain paragraph, no citation refs or
bibliography records. Raw carrier text is redacted, so this checks the digest
contract, not equality to independently extracted PDF text. No realistic lexical
feature or native extraction-accuracy claim is made.

Fresh diagnostic attempts after the summary/report change exited 0 for all three
PDFs. All pinned model resources loaded. On `1203.2295`, header processing refused
an existing zero font size; the reference stage processed 73 lines but admitted
zero records: one heading and 72 `NoPrintedLabel` dispositions. That is the
model's classification, not proof that the PDF lacks visible bracket labels.
There was no Body carrier run,
so the emitted body is empty, not a successful document-body extraction. The empty
citation feature digest does not prove citation-model inference. Its incomplete
inventory kept callouts unlinked. Ten summary limitations retained those counts. The other
two PDFs kept the font-statistics and missing-reference refusals above. All three
visual lanes reported Unsupported and heads returned Terminal; no images or
semantic figures are accepted by those runs.

The separate exact CLI diagnostic produced `U=3, H=1, L=2` with 49 matcher work
units. With budget 0 it returned `NotAssessable`, no L and no partial witness.
The budgets 100000 and 0 are invocation inputs, not changed production limits.
After the logging-only main changes, another `1203.2295` run and both exact
budgets exited 0. Their report/TEI bytes or parsed reports matched the earlier
diagnostics. The rolling JSON log retained all three composition-start events.
The same header/reference/body/visual limits still apply; no new model success
is inferred from that consistency check.

The preserved three-PDF body artifacts contain aggregate counts and digests, not
replayable ordered token inventories. No corpus L is inferred from those counts.
A corpus comparison requires actual source-bound inventories or a separately
specified extraction run.

## Remaining verification gates

The first normal 23-step architecture run stopped at the 20 GiB disk threshold
during test clippy. Formatter and library clippy passed; the owned paused process
tree was then terminated (exit -9). This is a resource-interrupted run, not a gate
pass. The second unchanged native run reached step 16 after the dependency-directory
relocation described below. Fourteen of steps 1–15 passed. Step 11 failed the
unchanged fulltext quarantine contract because the new emitter imported helper
functions from `native_fulltext_composition`. Step 16 was interrupted: the root
guard paused the owned tree at 19.71 GiB and killed it below its 15 GiB hard floor.
The run exited -9; steps 17–23 were not reached. Neither run is an architecture
pass. No step selection, cleanup hook or native timeout was changed. The separate
ALTO feature-clippy, governor-inventory and frozen visual-source binding failures
remain open even if a later architecture run passes.

The quarantine repair changes only the new candidate emitter and its contract
tests. It removes the composer dependency and redundant generic XML post-parse,
including its new-only `InvalidXml` error variant. This emitter never splices
caller XML: markup and names are fixed, source strings are escaped and checked
for XML 1.0 characters, IDs are unique `b{usize}` keys, and inline targets must
belong to that key set. Local escaping and text/attribute checks keep their prior
semantics. Identity checks, byte/count caps, duplicate-key refusals and link
checks are unchanged. Independent `roxmltree` property tests exercise generated
Unicode and adversarial markup-like text. The focused native execution passed
all 105 unchanged architecture contracts and all 10 candidate emission contracts.
An independent source review found no blocking defect in the closed-format repair.
On the repaired source, parse all-targets clippy, CLI example contracts (3 + 3),
feature example/lib clippy and the two plain runner builds passed. Three new PDF
diagnostics and the exact-complete diagnostic exited 0. The final parse fmt and
owned 12-crate fmt check passed. New hashes bind all 19 owned Rust files; only the
emitter and its contract differ from the prior post-logging stage. The current
source-index checks and subsequent full architecture run passed. Older receipts
retain their original source stage. The frozen composer, scanner, lifter and
quarantine test are unchanged.

The active trajectory check returned `drift_risk` with 18 flags and one violation,
not a green gate. It found missing pipeline logging in the two new example mains;
they now retain the existing `init_pipeline_logging` guard. The new example tests
passed again (3 + 3), the logging contract passed, feature example/lib clippy and
plain builds passed, and final scoped fmt/check passed. Active trajectory checks
after logging and after the complete architecture run no longer report the
missing-logging flag. The final check exited 0 but still returns `drift_risk`,
15 flags and one violation; it is not an all-green result. It reports 162 exec
captures and six quarantine runs past retention, compared with 30 and one in
the earlier check. No full retention plan was applied. The remaining
visual-positive artifact's converter-source hash is
stale after the bridge change. The artifact and its governor rule remain untouched;
that evidence binding is open. The coverage scorecard calls four new source files
untested, but the separately executed contract targets above cover them; the scorecard
result is not being suppressed or substituted for those receipts.

GitNexus refreshed its source-analysis index after the final boundary repair and
formatting. The follow-up found no path/ID mismatches across 107,542 path-bearing
nodes, indexed all 12 requested native symbols, and matched all 45 reported
changed-symbol IDs to raw rows. Both example-main calls to
`init_pipeline_logging` are indexed. No candidate-emitter CALLS edge to the
quarantined composer was found; direct source inspection also confirms the
import and helper calls were removed. No partial or truncated detect result was
reported. Upstream impact remains LOW for the with-limits emitter and `element`.
The wrapper returns UNKNOWN; literal checks confirm CLI, library re-export and
contract-test consumers rather than treating an empty caller set as unused.

This is not a complete call-graph claim: FTS is disabled, large artifacts are
skipped, and named callable-flow sites exceeded the index's candidate cap.
Explicit native queries cover the named new symbols; `detect_changes` alone does
not enumerate every untracked-file symbol. Earlier post-logging receipts retain
their original 107,537-node source stage.

No full hygiene plan was applied. The latest full plan included 53 exec captures
and one quarantine run; all remain present. After the instruction to proceed
without waiting, admission checked local ownership rather than stopping distinct
foreign workspaces. No daily target users were found. All 1,638 unit locks were
held exclusively during removal; no in-flight state, symlink or unaccounted foreign
hardlink was found. Protected sources, ledger probes and all 2,120 frozen hashes
matched before and after cleanup.

The existing governor application API applied only the normal policy-selected
`target/debug/incremental.kache-auto` subset. The cache is absent and the normal
manifest records zero evidence removals. Observed free space rose by 5,982,576,640
bytes (5.57 GiB) during that operation. A separate native `cargo clean -p da-cli`
removed only local derived CLI artifacts; Cargo reported 102 files and 1.5 GiB.
Both tested candidate/exact runners were hash-preserved outside target first.
Shared caches, foreign targets, models, source files and frozen evidence were not
cleaned. An independent read-only check confirmed the frozen/source/probe/review
hashes, retained runners, cache absence, zero-evidence manifest and all 160 excluded
paths still present. That check does not independently reconstruct historical
process locks or assert excluded-file content equality. Concurrent filesystem
activity means Cargo's removal total is not an independent free-space measurement.

The installed kache now reports `1.0.0`; the earlier capacity study inspected
`0.28.1` source. A stored `active_leases=1` is not a live-user count in that older
source, but that is not proof of the installed version's semantics. Cleanup relied
on current path, OS-lock, in-flight and process checks, not that counter.

After the interrupted tree exited, the complete owned `target/debug/deps`
directory was copied to `/dev/shm/daily-archive-m519-deps`, a separate session-named
location. Its mode remains 0775; the name does not imply access isolation. All 7,050 file
paths, sizes, modes, mtimes, SHA-256 hashes and intra-directory hardlink groups
matched. There were no symlinks. Required ledger-named deps entries were retained
on disk first; both top-level M518 probes and the tested M519 runners stayed on
disk with matching hashes. A temporary bind mount exposed the copied directory
at its original deps path. Native `readlink /proc/self/exe` probes confirmed that
executables keep that path; a directory symlink was rejected because tests derive
sibling example paths from their executable location.

Only the verified duplicate disk directory was then released. Observed root free
space rose by 9,150,148,608 bytes during that operation; concurrent filesystem
activity prevents attributing every byte to it. Sources, frozen evidence, ledger
stores, examples, build scripts and foreign targets were not relocated or removed.
The temporary bind mount was retained throughout the live build and test.
Build outputs in tmpfs were volatile; preserved evidence had durable copies
outside tmpfs. An independent read-only audit matched the stored complete
manifests, the then-current bind in the actual gate namespace and fresh protected
hashes. It did not rehash the changing live deps tree or independently reconstruct
the historical copy and lock operations. New required outputs needed durable
retention before teardown. Restoration required a quiet owned target, enough disk
space, a verified copy back and only then unmounting; its result is recorded below.

The second run started with 27.077 GiB free on root, 12.872 GiB free in tmpfs and
26.846 GiB `MemAvailable`. Its root guard retained admission 22 GiB, pause 20 GiB,
hard floor 15 GiB and two-second checks. Supplementary guards would abort only the
owned tree below 2 GiB tmpfs free or 4 GiB `MemAvailable`; they did not fire. Minimum
observed tmpfs free was 8.858 GiB and `MemAvailable` 16.774 GiB. These safety
thresholds do not change parser limits or gate timeouts.

After the owned run was killed, root free space fell further to about 0.29 GiB.
Read-only kache events attributed 10.609 GB of logical cache copies in the bounded
06:30–06:33:57 window to `/root/prime-agent`, versus 0.236 GB to daily-archive.
Logical writes are not net disk growth, but explicit foreign ENOSPC events and the
other session owner's report confirm concurrent pressure. That owner then cleaned
only their own target and reported no changes to daily-archive, its RAM mount,
evidence or the shared cache. Root free space recovered above 50 GiB. The parent
neither signalled foreign processes nor cleaned foreign targets or shared caches.
No further local incremental cache existed; a fresh normal-policy subset dry run
refused selection rather than deleting an unselected path. Those resource
interruptions remain failures. The later complete run demonstrates fit only for
its measured disk placement, invocation settings and concurrent workload; it is
not a general capacity guarantee.

After the external owner's recovery, the complete current deps directory was
restored to disk at a verified quiet boundary. The current inventory had 8,206
files and 15,318,655,151 logical bytes; the earlier 7,050-file manifest was not
reused for this operation. Newly generated ledger executables and metadata were
copied to durable storage first. A fresh disk staging copy matched the RAM
source in paths, types, sizes, modes, mtimes, SHA-256 hashes and intra-directory
hardlink groups. The source metadata remained unchanged and only the parent's
Cargo-lock descriptor referenced the local target.

After that comparison, native `umount` exited 0 and the verified disk staging
directory replaced the empty original mountpoint. A full hash comparison at the
restored original path matched again. All 2,120 frozen hashes, protected probes
and durable ledger copies matched. Only then was the redundant owned RAM copy
removed and the Cargo lock released. `target/debug/deps` is now a real disk
directory, not a bind mount or symlink. The restoration receipt records 37.712 GiB
root free and 36.511 GiB `MemAvailable` afterward. Concurrent activity prevents
using those readings as a net storage-saving measurement. An independent read-only
review found no restoration blocker: the three stored manifests, durable ledger
copies, protected hashes and current disk/no-mount observations matched. It did
not rehash the changing live deps tree. The ordering check uses the retained
finalizer implementation and completed result, not an independent timestamped
system-call trace.

The serial verification queue started only after that finalizer succeeded.
All ten commands passed: parse clippy, CLI contracts, CLI clippy, plain builds,
three PDF diagnostics, exact-complete, final parse fmt and the owned 12-crate fmt
check. This is scoped verification, not a full architecture pass. All three new
PDF report/TEI pairs and the exact-complete report matched the prior diagnostics;
the known extraction refusals remain. Source-index checks and fresh preservation
checks passed. The unchanged complete 23-step gate started at 07:07:42 UTC on
2026-10-15, PID 566382, with 38.207 GiB root free and 37.032 GiB `MemAvailable`.
It exited 0 after 5,268.980 seconds: **23/23 passed, 0 timed out, ARCHITECTURE
GATE: PASS**. All 19 source hashes still matched the final formatted bindings.
All native step timeouts remained 3,600 seconds. No cleanup hooks or skipped
steps were added. The final closed root-guard record has a minimum 32.089 GiB
free, with no pause or hard-floor abort; the supplemental guard observed minimum 30.820 GiB
`MemAvailable`, with no abort. These sampled minima are not a controlled peak
resource measurement or savings comparison.

The two earlier resource interruptions and the second run's genuine step-11
quarantine failure remain in their original receipts. This later pass covers
the repaired source; it does not relabel those earlier runs. The separate ALTO
feature-clippy, governor inventory and frozen visual-artifact source-binding
failures remain open. No current Task Attempt, scholarly-quality acceptance,
production-route replacement or promotion follows from architecture PASS.

All architecture invocations use `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0` and `CARGO_INCREMENTAL=0`, with jobs=1 and offline
Cargo. These invocation-only build settings alter artifact identity and can change
default codegen units from 256 to 16. They are not default-profile measurements.
The temporary tmpfs placement also changed cache hardlink/copy behavior; neither
scientific resource savings nor full-run peak usage has been established.

The final normal-policy `governor closeout` dry run exited 0. It reports
`target/` at 19.0 GB against a 60 GB cap, no selected build caches, 162 expired
exec captures, six quarantine runs and 12 package-clean commands. It would
reclaim 1.2 MB from stores plus the unmeasured package outputs. No `--apply`,
package clean, retention deletion or mass cleanup was performed after the gate.
These derived outputs and evidence remain available; this is not canonical
slice closeout.

## GSD status

M519 remains the selected work, not a completed milestone. The supported isolated
read at 08:39:40 UTC on 2026-10-15 found its legacy milestone status `queued`,
all four slices and five tasks pending, and their canonical lifecycles `ready`
at state version 0. Project revision was 7257, schema 51, authority epoch 0.
The read found zero running Task Attempts and zero latest succeeded/unpublished
verify-stage Attempts, both project-wide and for M519. It did not measure all
historical Attempts; those readers require the process-wide engine connection.
No normal migration/cutover connection was opened to obtain that count.

The installed completion path still requires a real host-claimed Attempt and
matching authority. Native execution receipts do not create an Attempt, host
Technical Verdict or completion. No receipt is relabelled as engine-owned proof;
no task is skipped or cancelled and no waiver is granted. The 60 retained
historical cutover refusals were not changed.

Effective host policy remains `isolation:none`, `gitops=true`, action `commit`
and push `true`. The current `uokEnabled=false` / `legacyFallback=true` values do
not disable closeout commit. No automatic host dispatch is started under the
current no-commit/no-push scope. An operator must establish and verify a supported
no-commit/no-push host mode before real canonical dispatch. This work does not
change those preferences or claim the missing authority.
