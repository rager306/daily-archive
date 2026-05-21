---
id: M021-xcfj4p
title: "Deterministic Candidate Locator Implementation and Ambiguity Diagnostics"
status: complete
completed_at: 2026-05-21T10:47:57.318Z
key_decisions:
  - Stable span hashes must use source ID, source hash, coordinate space, offsets, and route name, not local paths.
  - Overlap diagnostics are required for route-window ambiguity.
  - Next work should be chunk/section structure repair plus reviewer packets, not positive import.
key_files:
  - src/arxiv_archive/candidate_locators.py
  - tests/test_candidate_locators.py
  - .gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json
  - .gsd/milestones/M021-xcfj4p/slices/S04/independent-deterministic-locator-review.md
  - .gsd/milestones/M021-xcfj4p/slices/S04/run-evidence/final-deterministic-locator-guard.json
  - .gsd/milestones/M021-xcfj4p/M021-xcfj4p-VALIDATION.md
lessons_learned:
  - Independent review can find reproducibility issues that unit tests miss; fix bounded concrete gaps before closeout when possible.
  - Path-dependent hashes are not stable provenance identifiers.
  - Overlap diagnostics make ambiguity more actionable but do not establish semantic KG readiness.
---

# M021-xcfj4p: Deterministic Candidate Locator Implementation and Ambiguity Diagnostics

**M021 implemented and validated deterministic candidate locator generation with stable span hashes and overlap ambiguity diagnostics.**

## What Happened

M021 implemented deterministic candidate locator generation from the M020 protocol. It began with a design and impact map, then added an additive `candidate_locators.py` module and focused tests. The module builds review-only locator artifacts, validates forbidden payload keys and safety flags, handles source hash mismatches, reports broad-match ambiguity, and writes only validated JSON. S03 used it to generate a deterministic bounded batch over the 10 M011 targets. Independent review initially flagged path-dependent span hashes and missing overlap diagnostics; both were remediated. Final evidence: 12 tests pass, ruff clean, LSP clean, 10-paper deterministic batch, 26 locators, 20 ambiguous spans, 10 overlap diagnostics, 0 import-eligible locators, 0 fact promotions, and no import/write/raw-payload behavior. R049 validated; positive import remains blocked.

## Success Criteria Results

- Reproducible code: met.
- Explanatory ambiguity diagnostics: met with broad-signal and overlap diagnostics.
- Safety/no raw payload/no import tests and guards: met.
- Independent review and next-step recommendation: met.

## Definition of Done Results

- Deterministic locator module exists: met.
- Tests cover source hash verification, coordinate validation, ambiguity classes, forbidden payload rejection, and safety flags: met, 12 tests.
- Bounded batch run produced deterministic artifacts: met.
- Independent review completed and concrete findings remediated: met.
- No import/write/raw-payload behavior: met.

## Requirement Outcomes

R049 validated by deterministic implementation, tests, bounded batch evidence, independent review, remediation, and final guard. Positive KG import remains deferred.

## Deviations

S04 independent review found two concrete implementation gaps. They were fixed before final milestone closeout instead of being deferred.

## Follow-ups

Plan next milestone: Chunk Structure Repair and Reviewer Packet Prototype. Use deterministic locator output as input, keep positive KG import and LadybugDB writes blocked.
