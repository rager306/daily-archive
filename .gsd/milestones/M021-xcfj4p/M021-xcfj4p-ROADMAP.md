# M021-xcfj4p: Deterministic Candidate Locator Implementation and Ambiguity Diagnostics

**Vision:** Turn the M020 locator protocol into deterministic, testable code that produces safe candidate evidence and explains ambiguity before any semantic import gate is considered.

## Success Criteria

- M020 protocol becomes reproducible code, not hand-written artifacts only.
- Ambiguity diagnostics are more explanatory than M020's count-only labels.
- Tests and guards prove no raw corpus leakage, no import, and no LadybugDB writes.
- Independent review assesses semantic usefulness and next-step direction.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After S01, there is a designed code API for deterministic candidate locators with clear safety invariants and tests planned.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: After S02, unit-tested code can build candidate locator artifacts for source-backed targets with no import/write/raw payload behavior.

- [x] **S03: S03** `risk:medium` `depends:[]`
  > After this: After S03, a bounded batch run uses the implemented module to reproduce M020-style artifacts and richer ambiguity diagnostics.

- [x] **S04: S04** `risk:medium` `depends:[]`
  > After this: After S04, independent review decides whether deterministic locators are useful enough for reviewer packets/chunk repair next, still not positive import.

## Boundary Map

Not provided.
