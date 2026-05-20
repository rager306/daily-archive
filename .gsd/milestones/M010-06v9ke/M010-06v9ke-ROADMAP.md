# M010-06v9ke: Next Reviewed Plus Ten With Provenance Gates

**Vision:** Run exactly one next +10 validation batch under M009's provenance, lineage, and top-up gates.

## Success Criteria

- Next +10 selected with no overlap against M006/M008.
- Source-ready quota reaches 10/10 or blocks explicitly.
- Scan artifacts are active-lineage stamped and provenance-fresh verified.
- Independent review completes and gates next action.
- No positive import or production writes occur.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After this slice, a new +10 manifest exists with no overlap against M006 or M008 and a redacted availability report.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: After this slice, the selected batch has source-ready quota 10/10 or an explicit bounded shortage blocker.

- [ ] **S03: Run provenance verified next plus ten scan** `risk:high` `depends:[S02]`
  > After this: After this slice, the next +10 scan artifacts are active-lineage stamped and verified fresh by provenance.

- [ ] **S04: Review gated next plus ten evidence** `risk:medium` `depends:[S03]`
  > After this: After this slice, independent review decides whether evidence permits another gated batch, requires hardening, or blocks progression.

## Boundary Map

| Boundary | In scope | Out of scope |
|---|---|---|
| Corpus selection | Select one next +10 excluding M006 and M008 papers | Broad corpus scaling or run-to-100 |
| Source readiness | Preflight, bounded acquisition, bounded top-up if needed | Unbounded crawling/conversion |
| Provenance | Real provenance JSONL for init/preflight/scan or wrapper steps, verify-artifacts fresh | Fake/synthetic provenance for final claims |
| Scan | Markdown-based validation scan with active --milestone-id | Positive KG import, production LadybugDB writes |
| Review | Independent review of artifacts and gates | Semantic KG correctness certification |
