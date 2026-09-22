# daily-archive — Vision

*Created: 2026-09-22. Destination and principles; current shape lives in PRODUCT.md, direction in STRATEGY.md.*

## End state

A local-first scientific knowledge engine that turns a personal arXiv/PDF corpus into a trustworthy, queryable knowledge graph. Every paper is parsed by the native Rust cascade with conservation proof attached to every family; extraction is measured against gold, not asserted; the graph is unlocked by an explicit human decision and stays honest through schema, assurance and healing planes. Java GROBID is retired per family, remaining only as a rollback target.

## The path there

1. **Semantic correctness (now — M511).** The native composition must stop lying before it can replace: XML well-formedness as an Accepted precondition, bibliography ID/target integrity, field fidelity with typed lane fallback, visual typed absences, numeric corpus regression on fixed identity, measured resource ceilings, the Java→Rust→test parity ledger. Java stays the production oracle throughout.
2. **Parity and release.** Independent corpus numbers (H18), the frozen external holdout executed with margins (H20), and a recorded human release decision (GAR-015). Nothing here can be automated away: the release gate is a person.
3. **Per-family promotion.** One family at a time leaves `candidate` (lattice/segmentation → header atoms → bibliography atoms → body accounting → visual), each promotion carrying same-n evidence and its own receipts. Learning W2 starts feeding the policy FSM: recorded trajectories → ReasoningBank patterns → MicroLoRA policy candidates packaged as WASM, always fail-open to DATA defaults.
4. **Article assurance.** ACAP admits articles into downstream context only after parse receipts exist and are trusted.
5. **Graph unlock.** The last door, and not in this programme: import leaves `import_eligible=false` only on an explicit human yes, after honest parity — then the extraction, resolution and healing planes do their work on real graph truth.

## Principles

Specific to the destination (delivery boundaries live in STRATEGY.md):

- **Honesty over speed.** A green test count never substitutes for an independent check of the output; claims bind to a source revision.
- **Typed absence over silence.** Anything missing or unmeasured is a typed value (`no_claim`, `NotObserved`, `bytes_absent`), never a zero or an inference.
- **Fail-closed defaults.** Unknown rights, unknown backends, unknown stages block rather than guess.
- **Independent oracles.** Nothing scores itself: baseline and candidate producers differ from the oracle producer.
- **No invented facts.** Authors, DOIs, captions never come from a prompt; generative output stays non-authoritative candidate material.
