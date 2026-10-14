# M518 bounded vendor-update evidence

`raw-evidence.tar.gz` retains the bytes of the selected local receipts. `manifest.json`
lists their lengths and SHA256 hashes and the archive hash. Paths in this manifest
are archive member names, relative to `/tmp/vendor-upgrade-audit` during execution.
The archive is evidence, not an executable installer or a database backup.

Start with these members:

| Question | Members |
| --- | --- |
| Exact pins and upstream changes | `samyama-upstream.json`, `ruvector-upstream.json`, `samyama-persistence.md` |
| Current native gate | `full-architecture-after-content-repair.json`, `.log`, `full-architecture-after-content-repair-step-captures/` |
| Current product-source identity | `after-content-repair-pre-gate-source-binding.json`, `pre-closeout-source-and-original-recheck.json` |
| Actual typed UAT | `final-typed-uat-receipt.json`, `final-artifact-uat-result.jsonl`, `final-hermetic-ledger-uat-result.jsonl`, their stdout/stderr and metadata |
| Original-source preservation correction | `original-patch-full-byte-proof-correction.json`, `samyama-original-complete-before-upgrade.patch`, `user-temporary-scratch-cleanup.json`, `original-samyama-untracked-bytes/` |
| Native ledger compatibility | `ledger-old-to-new-compatibility.json`, `ledger-probe/`, `ledger-old-native-link.json`, `ledger-new-native-link.json` |
| Independent reviews | `final-source-review.md`, `review-followup.md`, `embedding-roots-repair-review.md` |
| Previous failures | `full-architecture-resource-interrupted.json`, `full-architecture-step-captures/`, earlier clippy/resource receipts |
| Graph scope and limits | `after-content-repair-graph-checks.json`, `after-content-repair-identity-detect.jsonl` |
| Requirements and workflow disposition | `m518-requirement-impact.md`, `final-current-gate-dispositions-result.jsonl`, `external-attemptless-slice-disposition-result.jsonl` |
| Resource retention decision | `native-governor-closeout.json`, `.log`, `native-governor-retention-decline.json` |

The current native architecture gate passed 23/23 with no timeout. Its 7,629 successful
test executions include repeated feature planes. They are not combined with the
878 earlier scoped executions, four copied-ledger cases or four fresh typed-UAT
ledger repetitions to claim unique tests. The two capacity boundaries ran five
supplemental commands each; their cleanup and exact default-plane restoration are
disclosed. Cargo ran offline, single-job and with invocation-only debug-info limits.
Unrestricted-profile equivalence is unproved.

Both old aggregate source captures are retained unchanged. Each is clipped and is
not a complete patch backup. The complete native original patch has 249 file
boundaries, 3,323,584 bytes and SHA256
`d79bb5221e03b2da4c4b49de8727a2d8ce35c66dd783d8c2601caf8a71146fcc`.
The correction binds 247 original Rust file hashes, two separate non-Rust diffs
and eight untracked byte copies. The original source tree was not changed.

No runtime graph/ledger store, compiled binary, cache, GitNexus database or frozen
M517 runner is copied here. Two unrelated cross-project process logs are excluded
because command lines contained secret-like fields; local originals remain private.
A bounded credential-pattern scan found no matches in the selected export. This is
not a universal security guarantee. Oversized source/tree/formatter reports remain
local, with their receipt hashes where recorded.

Some members are plans, prepared scripts or historical status reports. Their names
and raw bytes stay unchanged; inclusion never turns a draft into execution evidence.
Later actual receipts supersede earlier pending states without erasing failures.
The final current-source seal and GSD acceptance live in the engine and local
closeout receipts after this archive is fixed; they are not included recursively.

Scope: exact audited dependency commits, stated consumer contracts, copied
String-ordinal-0 JSON ledger compatibility and fresh current-generation contracts.
Not proved: generic bincode migration or downgrade, crash durability, MSRV 1.85,
GGUF inference, 10x capacity, universal graph coverage, optional Entity readiness,
model enforcement, historical SHA-only backfill or materializer-example runtime.
No deployment, import, automatic promotion, operational graph write or push.
