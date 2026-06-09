# S01: R and D Conflict Audit First

**Goal:** Audit all existing GSD Rxxx and Dxxx records before ADR drafting, classify conflicts against the universal-KB direction, GraphDB deferral, sidecar boundaries, and safety invariants, and produce a correction/discussion queue for later slices.
**Demo:** After this, all existing GSD requirements and decisions have been checked before ADR drafting, with conflicts routed to correction or user discussion.

## Must-Haves

- Reads current `.gsd/REQUIREMENTS.md` and `.gsd/DECISIONS.md`.
- Classifies every requirement and decision as `consistent`, `historical-scope-only`, `needs-clarification`, `superseded-by-new-ADR`, or `conflict-needs-user-decision`.
- Flags LadybugDB-finality wording, paper-only overfitting, parser-as-truth implications, unsafe graph/import implications, agent-orchestrator ambiguity, and contradictions with universal-KB direction.
- Produces machine-readable audit matrix and markdown report.
- Produces a correction/discussion queue for later slices.
- Does not silently rewrite historical decisions; append-only/superseding policy is preserved.

## Proof Level

- This slice proves: Machine-readable audit matrix plus markdown report and correction checklist verified by a local verifier.

## Integration Closure

S01 feeds S02-S07 so ADRs, PRD, contracts, and roadmap address known conflicts instead of discovering them at closeout.

## Verification

- Creates durable consistency surfaces for future agents before architecture writing begins: JSON audit matrix, markdown report, and verifier diagnostics.

## Tasks

- [x] **T01: Extracted the complete GSD requirement and decision inventory for M034 conflict auditing.** `est:small`
  Build a deterministic inventory from `.gsd/REQUIREMENTS.md` and `.gsd/DECISIONS.md`, preserving IDs, statuses, descriptions/decisions, choices, rationale, and source context where available. Output compact JSON and a counts report under the M034 decision package directory.
  - Files: `.gsd/REQUIREMENTS.md`, `.gsd/DECISIONS.md`, `.gsd/milestones/M034-kuei9y/decision-package/r-d-inventory.json`, `.gsd/milestones/M034-kuei9y/decision-package/r-d-inventory-summary.md`
  - Verify: Run a local verifier or script that proves inventory counts match parsed Rxxx and Dxxx IDs from the source files and no duplicate IDs are present.

- [x] **T02: Classified every GSD requirement and decision against the universal-KB ADR frame.** `est:medium`
  Classify every Rxxx and Dxxx record against the proposed universal-KB architecture, GraphDB deferral, sidecar/evidence boundaries, parser-as-candidate invariant, no-direct-GraphDB-write rule, and agent-boundary rule. Produce JSON and markdown audit artifacts with per-record categories and findings.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/r-d-consistency-audit.json`, `.gsd/milestones/M034-kuei9y/decision-package/R-D-CONSISTENCY-AUDIT.md`
  - Verify: Run a verifier that checks every inventory record has exactly one classification and that required risk categories are represented in the audit schema.

- [x] **T03: Created the correction and discussion queue for all S01 audit clarifications.** `est:small`
  Create a correction checklist and open-conflicts queue from the audit findings. Distinguish requirement updates, superseding decisions, deferred clarifications, and user-discussion items without mutating old decisions silently.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/correction-checklist.md`, `.gsd/milestones/M034-kuei9y/decision-package/open-conflicts-for-user.md`
  - Verify: Run verifier checks that every conflict/needs-clarification/superseded finding from the audit appears in either correction checklist or open-conflicts queue.

- [x] **T04: Added and verified the M034 R/D consistency audit verifier.** `est:small`
  Implement and run a local verifier for the S01 audit package, checking source coverage, classification coverage, conflict routing, safety invariant presence, and no silent mutation policy. Produce verifier output suitable for GSD closeout.
  - Files: `scripts/verify_m034_rd_consistency_audit.py`
  - Verify: `uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md`

## Files Likely Touched

- .gsd/REQUIREMENTS.md
- .gsd/DECISIONS.md
- .gsd/milestones/M034-kuei9y/decision-package/r-d-inventory.json
- .gsd/milestones/M034-kuei9y/decision-package/r-d-inventory-summary.md
- .gsd/milestones/M034-kuei9y/decision-package/r-d-consistency-audit.json
- .gsd/milestones/M034-kuei9y/decision-package/R-D-CONSISTENCY-AUDIT.md
- .gsd/milestones/M034-kuei9y/decision-package/correction-checklist.md
- .gsd/milestones/M034-kuei9y/decision-package/open-conflicts-for-user.md
- scripts/verify_m034_rd_consistency_audit.py
