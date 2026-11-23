# Archive — 001-orientation

Milestone: orientation
Shipped: 2026-11-23
Final verdict: all criteria met; project verify passed
Waves: 3  Tasks: 5 done / 5 total  Review cycles used: 1/1/1
Carried forward: 21 DOCS-AUDIT ruling(s)

## Success criteria at ship

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| SC1 — doc/review/architecture-map.md: every architecture/dependency claim cites a concrete path in crates/ or Cargo.toml; no claim line without a path citation (checked over the whole file). | met | doc/review/architecture-map.md:24,55,64,73 |
| SC2 — doc/review/functionality-ledger.md: closed enumeration — every subcommand from the dispatch match in crates/da-cli/src/main.rs plus every use-case in crates/da-application — each with one verdict from exactly {работает, кандидат, заглушка, unverified} and an evidence path; no dispatch entry omitted. | met | doc/review/functionality-ledger.md:175 (sample row), crates/da-cli/src/main.rs (dispatch match) |
| SC3 — doc/review/grobid-state.md contains four sections: (a) production route + identity (GROBID 0.9.1, D576) with citations; (b) table of all R123 ladder families (lattice/segmentation, header, bibliography, body accounting, visual) with per-family status shadow/candidate/promoted and evidence paths; (c) retirement-package status; (d) exhaustive list of gates separating current state from any production flip, each with a feasibility note. | met | doc/review/grobid-state.md:1-28,§(b),§(c),§(d); artifacts/parse_compat/grobid_retirement_acceptance_package.json (blocking_gates) |
| SC4 — all three files are committed on branch gsd-path/M001; the commit diff touches only .md files. | met | .project/review/final-gap-1.md; .project/build/verify-ledger.jsonl |

## Contents

- build/evidence.json
- build/verify-ledger.jsonl
- intent/INTENT.md
- plan/PLAN.md
- research/DOCS-AUDIT.md
- research/RESEARCH.md
- research/SYNTHESIS.md
- research/evidence-codebase.md
- research/evidence-pitfalls.md
- research/evidence-retirement-gates.md
- review/FINAL.md
- review/PATCH-FINDINGS.md
- review/final-gap-1.md
- review/final-gap-2.md
- review/wave-1.cycle1.md
- review/wave-2.cycle1.md
- review/wave-3.cycle1.md
- tasks/T001-grobid-state-report.md
- tasks/T002-architecture-map.md
- tasks/T003-functionality-ledger.md
- tasks/T004-fix-doc-remediation.md
- tasks/T005-node-schema-count-fix.md

## Notes

- none
