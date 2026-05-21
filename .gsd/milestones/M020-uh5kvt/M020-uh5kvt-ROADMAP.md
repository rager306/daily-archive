# M020-uh5kvt: KG Candidate Locator and Chunk-Span Provenance Protocol

**Vision:** Retire the next Scientific KG blocker by creating and testing candidate locators with exact chunk-span provenance under protocol-bound, review-gated safety rules.

## Success Criteria

- Candidate locator protocol exists and is validated before use.
- One-paper and small-batch locator artifacts demonstrate source-span provenance without fact promotion.
- Independent review assesses semantic usefulness rather than count-only success.
- No Scientific KG production import or LadybugDB writes occur.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After S01, daily-archive has a protocol contract for candidate locators, source spans, uncertainty, review queues, and import-disabled safety flags.

- [x] **S02: S02** `risk:medium` `depends:[]`
  > After this: After S02, one known paper has candidate locators with exact chunk-span/source-span provenance under the S01 contract and import disabled.

- [x] **S03: S03** `risk:medium` `depends:[]`
  > After this: After S03, a bounded small batch reports locator coverage and failure modes without enabling import.

- [x] **S04: S04** `risk:medium` `depends:[]`
  > After this: After S04, independent review decides whether candidate locators are meaningful enough to justify a future positive import-gate milestone.

## Boundary Map

Not provided.
