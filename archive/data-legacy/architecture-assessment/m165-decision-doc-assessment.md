# M165 Decision and Documentation Assessment

## Verdict

**Decision/documentation verdict: SUBSTANTIALLY ALIGNED, WITH STALE ENFORCEMENT TEXT.**

The binding architecture decisions are directionally correct and the implementation now matches the main D086/ADR-034 intent better than it did in M163. The main governance gap is that some documentation still describes the pre-M164 guard scope and evidence counts even though the live guard now scans four layers and reports zero strict-boundary debt.

## Evidence

Reviewed sources:

- `.gsd/DECISIONS.md` around D086/D087/D088.
- `doc/adr/ADR-034-hexagonal-onion-overlay.md`.
- `doc/onion-layers.md`.
- `data/architecture-assessment/m164-contract-classification.md`.
- `data/architecture-assessment/m164-closeout.md`.
- Live guard evidence from S01.

## Alignment strengths

### A1 — D086 remains the correct governing decision

D086 explicitly chooses:

- hexagonal Ports/Adapters plus onion layering,
- `domain/application/infrastructure` as target structure,
- `typing.Protocol` Ports in domain only when justified,
- Adapters in infrastructure,
- Ponytail Port rule to prevent symmetry-only Ports.

Current code is aligned: domain/application/infrastructure/workflows import rules are clean, Ports/Protocols exist in domain/application/infrastructure according to the ADR taxonomy, and M164 moved misplaced contracts inward.

### A2 — D087 still supports future async/queue evolution

D087 rejects Prefect now and keeps `DispatchProtocol`/`SyncDispatch`/`QueueDispatch` as the evolution seam. This remains compatible with current architecture: future queue activation should be an adapter/dispatch strategy, not a rewrite of the Core.

### A3 — D088 remains aligned with Ponytail Port discipline

D088 replaced premature `PDFParserPort` with `FullTextProviderPort` around real MDConverter multi-backend behavior. This is still a good example of the Port rule: introduce Ports only where runtime variability, migration, or mockability exists.

### A4 — M164 classification was disciplined

`m164-contract-classification.md` avoided speculative Ports and classified each strict-boundary debt item by ownership:

- pure safety contracts to domain,
- validation DTO/provenance to application,
- validation logging and quality gate to infrastructure,
- script logic to package modules with scripts as wrappers.

This matches both D086 and Ponytail.

## Documentation drift

### D1 — `doc/onion-layers.md` still describes old guard scope

The “What the guard enforces” section says the guard AST-scans only `domain/` and `application/`. That is stale after M164. The live guard now scans:

```text
domain
application
infrastructure
workflows
```

Risk: **medium**. Future agents may under-estimate guard coverage or misread the intended boundary matrix.

### D2 — ADR-034 has stale enforcement language and evidence counts

ADR-034 still contains language like:

- guard scans `domain/` and `application/`,
- evidence table mentions old file counts such as domain 8/application 6,
- summary says multi-layer guard with “domain+application clean”.

The decision remains valid, but the enforcement details lag current implementation.

Risk: **medium**. The ADR is accepted and binding, so stale implementation details carry more governance weight than ordinary docs.

### D3 — Compatibility shim lifecycle is not written as a decision

M164 intentionally preserved compatibility shims. That is pragmatic, but there is no explicit decision that says:

- old shim modules are compatibility-only,
- new production imports should target canonical homes,
- shims have a removal or ratchet strategy.

Risk: **medium**.

### D4 — Async/thread readiness policy is in docs, but not yet an ADR-level decision

`doc/onion-layers.md` now contains good policies for:

- async-first entrypoints,
- artifact write safety,
- adapter ownership/lifecycle.

Those policies are important enough for future architecture work, but they are currently living-doc policy, not an explicit ADR or GSD decision.

Risk: **medium**, especially before queue activation or multithreaded execution.

## Gaps

| Gap | Severity | Why it matters |
|---|---|---|
| Stale guard scope text in docs | Medium | Agents may plan from obsolete enforcement assumptions. |
| ADR-034 evidence table stale after M164 | Medium | Binding ADR should not contradict live guard scope. |
| No explicit shim lifecycle decision | Medium | Compatibility shims can become canonical by accident. |
| Async/write/lifecycle policy not promoted to decision | Medium | Future concurrency work needs durable governance, not just local doc text. |

## Backlog

| Priority | Item | Rationale |
|---|---|---|
| P1 | Update `doc/onion-layers.md` “What the guard enforces” to include infrastructure/workflows | Removes immediate doc drift. |
| P1 | Amend ADR-034 or add ADR-034 addendum for M164 strict-boundary enforcement | Keeps binding architecture record accurate. |
| P1 | Record a GSD decision for compatibility shims as deprecated facades | Prevents shim regrowth. |
| P1 | Record a GSD decision or ADR addendum for async/thread readiness policy | Makes write-safety and lifecycle rules durable before queue activation. |
| P2 | Add a short architecture status table to `doc/onion-layers.md` with current guard layer counts | Reduces future re-assessment friction. |

## Final decision/documentation conclusion

The repository’s architecture decisions are coherent and mostly implemented. The docs need a post-M164 synchronization pass before claiming governance-level strict compliance. The architecture itself moved forward; the written governance has not fully caught up.
