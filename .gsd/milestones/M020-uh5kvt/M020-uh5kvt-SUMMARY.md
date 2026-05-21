---
id: M020-uh5kvt
title: "KG Candidate Locator and Chunk-Span Provenance Protocol"
status: complete
completed_at: 2026-05-21T09:36:22.396Z
key_decisions:
  - Candidate locators are review evidence, not KG facts.
  - Positive import-gate work is deferred after independent review flagged 27/35 ambiguous locators.
  - Next work should implement deterministic locator generation with schema validation, source hash checks, coordinate validation, safety guards, and ambiguity diagnostics.
key_files:
  - .gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md
  - .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json
  - .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json
  - .gsd/milestones/M020-uh5kvt/slices/S04/independent-semantic-review.md
  - .gsd/milestones/M020-uh5kvt/slices/S04/run-evidence/final-locator-protocol-guard.json
  - .gsd/milestones/M020-uh5kvt/M020-uh5kvt-VALIDATION.md
lessons_learned:
  - Protocol and coordinate coverage are necessary but not sufficient for semantic KG import readiness.
  - High ambiguity is useful diagnostic evidence and should drive deterministic locator implementation rather than fact promotion.
  - A FLAG review can be a successful safety outcome when it blocks unsafe import and defines the next step.
---

# M020-uh5kvt: KG Candidate Locator and Chunk-Span Provenance Protocol

**M020 validated a safe candidate locator protocol and deferred positive KG import pending deterministic locator implementation.**

## What Happened

M020 resumed the Scientific KG mainline after the research-agent spike. It defined a candidate locator and chunk-span provenance protocol, exercised it on one source-backed paper, scaled it to the 10-paper M011 target batch, and subjected the result to independent semantic review. The final evidence shows the protocol is safe and useful for review-only locator work, but current heuristic locators are too ambiguous for positive import. Final metrics: 10 papers, 35 locators, 27 ambiguous spans, 0 missing spans, 0 conflicting evidence, 0 import-eligible locators, and 0 fact promotions. R048 was validated for protocol/rehearsal evidence, while positive KG import and LadybugDB writes remain blocked.

## Success Criteria Results

- Candidate locator protocol exists and is validated before use: met.
- One-paper and small-batch locator artifacts demonstrate source-span provenance without fact promotion: met.
- Independent review assesses semantic usefulness rather than count-only success: met.
- No Scientific KG production import or LadybugDB writes occur: met.

## Definition of Done Results

- Candidate locator protocol contract exists: met.
- One-paper fixture exists and passes guard: met.
- Small-batch rehearsal exists and passes guard: met.
- Independent semantic review completed: met, verdict FLAG for positive import readiness.
- No production import/write/raw-payload behavior: met.

## Requirement Outcomes

R048 validated by M020 protocol, guards, one-paper fixture, small-batch rehearsal, independent review, and final guard. Positive import remains deferred pending deterministic locator implementation and ambiguity diagnostics.

## Deviations

None. The independent review FLAG was expected as a safety-preserving outcome because M020 was protocol/rehearsal work, not positive import readiness.

## Follow-ups

Plan next milestone: Deterministic Candidate Locator Implementation and Ambiguity Diagnostics. Do not start positive import-gate work until deterministic locators reduce ambiguity and independent semantic review passes.
