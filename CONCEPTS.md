# CONCEPTS.md

Defines the words that mean something specific in this codebase — substrate that `AGENTS.md`, plans and learnings can cite without redefinition. Created 2026-09-22 as a seed from the declared domain models (parse plane, absorption programme, delivery engine).

## Knowledge graph and pipeline

**Samyama graph**
The single authoritative persistent store for the project's knowledge graph; no other substrate may hold graph truth.
*Aliases: Samyama, KG.*

**Import lock**
The fail-closed rule that nothing enters the knowledge graph until an explicit human release, surfaced as `import_eligible=false` (decision D127).
Every wave and milestone inherits it; no automated step may flip it, and lifting it requires a recorded human decision.

**Pipeline stage ledger**
The DATA-declared registry of pipeline stages (currently 14), each recording what context the stage adds and loses, its hard invariants, cache mode and failure class.

**Semantic graph task suite**
End-to-end checks that run a parse-derived snapshot against graph tasks in one process and grade each task PASS, EMPTY or ERROR; an EMPTY result fails the suite.

## GROBID absorption

**Capability ledger**
The registry of GROBID capabilities (currently 38), each in exactly one capability state, tracking absorption atom by atom across endpoints, dialects, internal stages and learning surfaces.

**Capability state**
The lifecycle value of a ledger row: `proxy` (Java HTTP called in production), `fallback` (handled inside the Java cascade), `shadow` (native code exists, disconnected from production), `candidate` (native with admission evidence), `absent` (not implemented), `production` (native and serving production).
Production is a state count, never a migration percentage; `shadow` and `absent` are different claims — code exists versus it does not.

**Conservation**
The accounting guarantee that a parse or composition consumed and re-emitted the source without silent loss, reported as family receipts and a `no_silent_loss` verdict.
An `Accepted` composition without conservation accounting is not trustworthy; loss must be typed, never silent.

**Family receipt**
The per-family record of what a parse stage consumed and produced (header, bibliography, body, visual, …) — the unit conservation is audited against.

**Three automata**
The separation rule that parsing, policy, and article assurance are three state machines that never share state: the parse workflow (PDF to family receipts), the policy FSM (routing, admission, retry decisions), and ACAP (article-stage admission after receipts exist).

**Policy FSM**
The decision machine that answers routing, admission, concurrency and retry questions for parsing, stamped as `policy_hash` or explicitly `policy=none`.
Its outputs are closed enums; it may never emit parsed content such as authors, DOIs or TEI fields.

**ACAP**
The Article-Context Assurance Plane: the automaton that admits articles into downstream context only after parse receipts exist and are trusted. Deferred until the parse waves close.

**Honesty gate**
A fail-closed check that blocks claims not backed by machine-verifiable evidence — backend identity, policy stamps, silent loss, denominator honesty, dependency boundaries.

**Wave**
A sequenced slice-set of the absorption programme (W0–W6 under M465) where each wave closes function first, then sequence, then same-input replay; W0 is the honesty condition under which later waves may exist, not a parser.

**Horizon stage**
A numbered follow-up item (H01–H21) in the absorption registries; `claim_scope` separates planning records, which may never claim PASS, from evidence records, which require re-executed proof.

**Stage tape**
An offline, verifiable recording of a real GROBID cascade invocation at stage granularity, used as oracle input instead of a live Java call.

**External holdout**
The frozen, checksum-pinned document set reserved for release measurement, executed with acceptance margins and cost accounting; distinct from any pilot run.

**Same-n comparison**
A baseline-versus-candidate evaluation where both sides run on identical inputs under identical names; required before any promotion.

**Parity ledger**
The three-link mapping every absorbed Java method must carry: Java source, Rust surface, independent test.

**Quarantine scanner**
A test or governor check that fails when a shadow-only symbol, token or route appears in a production surface.

## Delivery

**GSD**
The milestone/slice/task delivery engine that owns planning artifacts and lifecycle writes; hand-editing its database or state files is forbidden.
*Aliases: Git Ship Done.*

**Slice**
A delivery unit inside a GSD milestone with its own plan, tasks and acceptance.
Slice IDs are milestone-local: M511's S02 is not M465's S02.

**Auto run**
GSD's unattended mode, which dispatches units without a human in the loop under soft/idle/hard budgets and a per-run log.

**Wedge**
An engine-blocked state after repeated no-progress completions; lifted only with an explicit resume-wedge acknowledgment.

**Governor**
The repository's governance CLI: derives trajectory verdicts (`on_track`, `drift_risk`, `blocked`) from ADR rules and DATA registries, and runs the honesty reviews over parse-plane and absorption registries.

**Human gate**
A decision point only a named person may close — import release, external-holdout release review, promotion. Automation may prepare evidence but never record the decision.

## Flagged ambiguities

- "S02" was used for both the M465 wave slice and the M511 slice — slice IDs are always milestone-local.
- "Shadow" had been loosely used for "not implemented" — `absent` is that; `shadow` means implemented but disconnected from production.
- "Migration percentage" is a banned phrasing: production is a state count (C01), not a percent.
