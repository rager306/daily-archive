# M187 Transition Ratchet Decision

## Verdict

**M187 activates `transition-ratchet` mode for the four remaining manifest residuals.**

## Prior mode

M186 closed in `preserve-ratchet` mode. Under that mode, residual wiring was blocked because the strict inventory was required to remain:

- `script-only=4`
- `unknown=0`
- `shared-state=0`

M186 S10 proved that wiring can pass behavior while changing strict drift from `script-only=4` to `script-only=3`, so movement without a transition decision is contradictory.

## Active M187 decision

M187 switches the manifest residual work to `transition-ratchet` mode. Residual wiring is allowed only when each edit is preceded by exact GitNexus impact and followed by focused tests plus strict drift explanation.

## Target residuals

| Residual | Symbol | Batch |
|---|---|---|
| `m055-five-pdf` | `Function:scripts/benchmark_m055_corpus_manifest.py:build_corpus_manifest` | S02 |
| `m055deep-20-pdf` | `Function:scripts/build_m055deep_corpus_manifest_20.py:write_manifest` | S02 |
| `m058-graph-manifest` | `Function:scripts/m058_build_graph_manifest.py:write_json` | S03 |
| `m059-batch-manifest` | `Function:scripts/m059_build_manifest.py:finalize_manifest` | S03 |

## Preconditions before each source edit

- exact GitNexus impact by symbol UID,
- explicit risk review for MEDIUM or higher,
- focused residual tests identified,
- rollback boundary understood.

## Required proof after each movement batch

- focused residual tests pass,
- manifest contract tests pass,
- strict drift delta is explained,
- `unknown=0`,
- `shared-state=0`.

## Baseline policy

Canonical inventory baseline update is allowed only in S04 after behavior proof for the movement batches. It must encode only the intended residual-retirement delta.

## Forbidden shortcuts

- No broad write-path classification rules.
- No canonical baseline update before behavior proof.
- No unscoped replacement of arbitrary manifest or JSON helpers.
- No parser/chunk/graph readiness claims.
