# ADR-035: Write Path Governance and Canonical Inventory Baseline

**Status:** Accepted (binding)  
**Date:** 2026-06-27  
**Deciders:** collaborative  
**Milestone:** M183-lg1xjb  
**Scope:** architecture / write-path inventory / CI drift / scripts / cache lifecycle  
**Binding Level:** binding  
**Revisable:** yes, with implementation evidence

## 0. One-line Decision

`daily-archive` treats the write-path inventory as an architecture governance surface: scanner categories are added only by exact reviewed source paths, the committed canonical inventory baseline is the CI baseline, all scanner movement is proven by generated deltas, scripts remain process-boundary wrappers unless promoted through package seams, and cache/index/manifest movement requires exact lifecycle, invalidation, consumer, and concurrency proof.

## 1. Context

M169-M183 progressively reduced ambiguous write-path ownership while preserving architecture guardrails:

```text
unknown=0
shared-state=0
dynamic=0
legacy=0
onion violations=0
```

The important learning is not only that the residual `script-only` bucket shrank. The durable architecture rule is that movement is safe only when it is exact, reviewed, and verified against the canonical baseline. Broad prefix or target-name rules can hide real ownership problems and are therefore forbidden.

## 2. Decision

### 2.1 Exact source-path scanner policy

A new inventory category may be added only when all of the following hold:

1. The source path or small source-path set is reviewed.
2. The category name describes the reviewed ownership meaning.
3. Focused tests cover every selected path.
4. Focused tests also cover future unlisted paths that must remain `script-only`.
5. Generated deltas prove the expected movement and no unexpected drift.

Forbidden shortcuts:

- broad prefixes such as `verify_*`, `benchmark_*`, `audit_*`, or `m0xx_*`;
- generic target names such as `path`, `output_path`, `summary_path`, `report_path`, `destination`, or `cache_path`;
- broad cache, manifest, markdown, converter, or index rules.

### 2.2 Canonical baseline update protocol

The committed canonical inventory baseline is the CI baseline. A scanner/category change follows this protocol:

```text
1. Generate a fresh milestone baseline before edits.
2. Implement exact scanner movement and focused tests.
3. Generate movement delta from the milestone baseline.
4. Confirm unknown=0 and shared-state=0.
5. Refresh canonical JSON, markdown, and delta artifacts only after movement is proven.
6. Run strict canonical drift and require total/category delta +0.
7. Run architecture guards, quality stack, and GitNexus detect_changes.
```

The canonical baseline must fail closed before refresh when scanner categories intentionally change. Passing strict drift after refresh is the proof that CI is aligned again.

### 2.3 Script boundary contract

Scripts are process-boundary wrappers. A script write path may remain `script-only` when it is historical, one-off, or caller-owned evidence generation. Reusable behavior belongs in package modules behind domain/application/infrastructure seams.

A script output category does not make the script a reusable module. It only records that a specific reviewed output family has known ownership for inventory governance.

### 2.4 Cache/index/manifest proof gate

Cache, index, and manifest-looking writes remain conservative unless all proof exists:

1. exact source ownership;
2. stable lifecycle owner;
3. invalidation semantics;
4. consumer contract;
5. concurrency/write coordination behavior.

If any proof is missing, no-move is the correct outcome. Do not classify by `cache`, `index`, `manifest`, or related target names.

## 3. Options Considered

### Option A: Exact source-path governance

| Dimension | Assessment |
|---|---|
| Complexity | Medium |
| Safety | High |
| Drift visibility | High |
| Maintenance | Predictable |

**Pros:** preserves guardrails, exposes residual ambiguity, makes category movement auditable.  
**Cons:** slower than broad rules; requires repeated small waves.

### Option B: Broad prefix and target-name rules

| Dimension | Assessment |
|---|---|
| Complexity | Low |
| Safety | Low |
| Drift visibility | Poor |
| Maintenance | Risky |

**Pros:** reduces counts quickly.  
**Cons:** hides unrelated ownership semantics and can mask shared-state/cache bugs.

### Option C: Freeze scanner categories

| Dimension | Assessment |
|---|---|
| Complexity | Low |
| Safety | Medium |
| Drift visibility | Medium |
| Maintenance | Stagnates |

**Pros:** avoids accidental reclassification.  
**Cons:** leaves known reviewed evidence outputs ambiguous forever.

## 4. Consequences

- Future scanner work happens in small exact waves.
- Canonical baseline refresh is coupled to generated movement proof.
- Cache and manifest-looking paths remain conservative until lifecycle proof exists.
- Active docs now give future agents a single rule source instead of requiring them to infer policy from milestone artifacts.

## 5. Action Items

1. Keep adding exact residual script waves only when reviewed source paths form a real ownership family.
2. Keep canonical baseline refresh coupled to generated deltas and strict drift.
3. Treat cache/index/manifest movement as no-move unless the proof gate is satisfied.
4. Prefer promoting reusable script behavior into package seams over broadening script categories.
