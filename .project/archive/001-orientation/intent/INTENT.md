# Intent — daily-archive: ориентация и оценка состояния

<!-- Written by /skill:gsd-path-define (standard, brownfield, supplied-spec).
     Constraints and vetoes here override everything downstream. -->

Lane: standard   <!-- 3 deliverable-sized reports + fix-doc волна в одной волне — больше двух задач, lane standard. -->

Review panel: off   <!-- configured future preference: off (path_config show). -->

Finding skeptics: off   <!-- default; not quick lane, but assessment artifacts are reviewed by wave review. -->

Surfaces: doc/review/architecture-map.md, doc/review/functionality-ledger.md, doc/review/grobid-state.md

## Summary

Owner needs a verified orientation over the daily-archive repository: architecture, implemented functionality, and the current state of the GROBID implementation, before any further milestone decisions (M521-carry, promotion moves). The first release of this milestone is three evidence-cited reports committed to doc/review/ — architecture map, functionality ledger (CLI commands × application use-cases), GROBID state report — produced with the codebase-memory-mcp toolset against the fresh graph index (675k nodes at 0f06b764c) plus static reading and already-recorded run evidence. Supplied spec: /root/da-path-migration-20261122/IMPORT.md (recorded here per the define-entry instruction).

## Problem

The repository carries 23 milestones of gsd-pi-era history whose planning state is frozen; the governing engine changed to gsd-path on 2026-11-22. No current, code-verified single view exists of what is actually implemented, what is candidate, and where the GROBID→native replacement really stands — historical claims are scattered across core/*.md ledgers, ADRs, and review docs, several already stale (DOCS-AUDIT: 23 non-verified rows). Decisions made on stale claims risk wrong scoping.

## Users

The owner (single user, technical, drives the project through agent sessions) and downstream Path phases (research/plan read the reports as ground truth). No external users.

## Success criteria

1. SC1 — doc/review/architecture-map.md: every architecture/dependency claim cites a concrete path in crates/ or Cargo.toml; no claim line without a path citation (checked over the whole file).
2. SC2 — doc/review/functionality-ledger.md: closed enumeration — every subcommand from the dispatch match in crates/da-cli/src/main.rs plus every use-case in crates/da-application — each with one verdict from exactly {работает, кандидат, заглушка, unverified} and an evidence path; no dispatch entry omitted.
3. SC3 — doc/review/grobid-state.md contains four sections: (a) production route + identity (GROBID 0.9.1, D576) with citations; (b) table of all R123 ladder families (lattice/segmentation, header, bibliography, body accounting, visual) with per-family status shadow/candidate/promoted and evidence paths; (c) retirement-package status; (d) exhaustive list of gates separating current state from any production flip, each with a feasibility note.
4. SC4 — all three files are committed on branch gsd-path/M001; the commit diff touches only .md files.

## Edge coverage

| Edge | Criterion | Category | Disposition | Detail |
|---|---|---|---|---|
| E1 | SC1 | precision | criterion SC1 | Every cited path must exist at HEAD 0f06b764c; enforced as a Verify-side git-enumeration command over the finished report's cited paths (Correction C2). |
| E2 | SC1 | boundary | criterion SC1 | Absence claims (e.g. "legacy/ отсутствует") cite the checked location — stated in SC1's path-citation rule. |
| E3 | SC2 | precision | criterion SC2 | Verdict vocabulary is exactly {работает, кандидат, заглушка, unverified}; one verdict per command; enforced as a Verify-side vocabulary grep over the finished ledger (Correction C2). |
| E4 | SC2 | boundary | criterion SC2 | Enumeration is closed over the dispatch match in main.rs (help-only/hidden subcommands included), not over README. |
| E5 | SC3 | boundary | criterion SC3 | Family set is exactly the R123 ladder; anything found beyond it gets its own candidate row. |
| E6 | SC3 | empty | criterion SC3 | A family with no code gets a typed-absent row, never a silent skip. |
| E7 | SC4 | boundary | criterion SC4 | Commit lands exactly on gsd-path/M001; diff paths are .md only. |

## Prohibitions

| Prohibition | Criterion | Must not | Disposition | Detail |
|---|---|---|---|---|
| N1 | SC4 | assessment must not become promotion: no pin, threshold, or status value in code/config changes | criterion SC4 | git diff shows only .md files. |
| N2 | SC3 | grobid-state must not declare a native family production-ready without promotion evidence | criterion SC3 | Any "ready" wording carries a gate-receipt citation; without it the row stays candidate. |
| N3 | SC4 | fix-doc edits must not leave .md files | criterion SC4 | git diff path check. |
| N4 | SC1 | the map must not restate historical doc claims as current code facts: Proposed ADRs (042–049) and removed crates (da-graph, legacy/) may appear only explicitly marked as proposed/removed | criterion SC1 | Every implementation claim cites a code path at HEAD; doc-only claims are labeled as documentary. |
| N5 | SC2 | a ledger verdict must not silently become «работает» without execution evidence | criterion SC2 | Every «работает» cites ran-in-gate or recorded-run evidence; otherwise кандидат/заглушка/unverified. |

## Scope: in

- Three assessment reports in doc/review/ per SC1–SC3, produced with the codebase-memory-mcp toolset (search_graph, trace_path, query_graph, get_architecture, get_code_snippet, get_file_outline, search_code, check_index_coverage) + static reading + recorded run evidence (gate 23/23 log, preflight reports, existing artifacts).
- Mechanical fix-doc wave: apply the stale-doc corrections already classified in .project/research/DOCS-AUDIT.md remediation rows 1–9, 11–14, 16–22 (counts, paths, dead legacy/ references) — .md only.
- Verdict proof-weight tags on every ledger entry: ran-in-gate / static-only / unverified.

## Scope: out (vetoes)

- Import unlock / graph writes / deployment / auto-promote / production_route flip — verbatim hard locks D127 + ADR-076 (`import_eligible=false`, `deployed=false`, `graph_write=forbidden`, `auto_promote=forbidden`, `production_route=forbidden`, `thresholds=not_accepted`); nothing in this milestone may research, plan, or build them.
- Free invention beyond constrained select (`candidate_id` only); GEPA/LLM auto-promote; sidecar TEI/MD as graph truth; non-Samyama state stores; DSPy enablement (D136); onion inversion (kg-* → da-*).
- M521-carry (ADR-081 lexicon rights, refreshed pins) — postponed by owner correction C1; not part of M001.
- Code or config changes under the cover of documentation work (N1/N3).
- Re-derivation of extraction eval metrics on the gold corpus (owner chose static+logs evidence budget; runs belong to a later milestone).

## Constraints

- Stack: existing Rust workspace (do not introduce code); assessment read-only except .md edits.
- Method: codebase-memory-mcp full toolset against the refreshed index (owner instruction, 2026-11-22); recorded evidence allowed (gate log /tmp/arch-20261123-pass2.log, preflight /tmp/preflight-20261122.log), no new cargo/test runs.
- Branch: gsd-path/M001 bound at 0f06b764c; integration direct; default branch master (origin/master = 0f06b764c).
- Governance: GROBID 0.9.1 = current identity (D576), 0.9.0-crf = rollback target; D572–D575 rulings carry; new work obeys D237 (no new Python outside governor/tests roots).
- Docs-audit rulings bundle (2026-11-22, owner): rows 1–9, 11–14, 16–22 fix-doc (mechanical, now in-scope as .md edits); row 10 fix-doc (align ".project/ is committed" wording to pipeline-committed reality); rows 15, 23, 24 accept-drift (resolved by router initialization / MCP-managed mirror / frozen engine, verbatim in DOCS-AUDIT.md User rulings).

## Current state (brownfield)

- **What exists**: Rust workspace, 12 crates (kg-{ontology,storage,embeddings,algorithms,pipeline} + da-{domain,ports,application,parse,adapters,cli,mcp}), 532,770 crate LOC, 5,012 tests + 791 tokio tests, 638 locked packages; storage = Samyama 1.16 (path dep /root/vendor-source/samyama-graph) + RuVector; parse plane dominates (da-parse 65,395 src LOC, 146 integration suites); GROBID 0.9.1-crf Java = production parse route; native route exists as gate-verified candidate wave (23/23, 0f06b764c).
- **Verified recently**: full architecture gate 23/23 PASS (2026-11-23, first complete pass since October); codebase-memory index refreshed (675,624 nodes / 924,867 edges, wave-inclusive); preflight snapshot with coverage_delta ok.
- **Known weak spots**: vendor path deps bind builds to /root/vendor-source (self-hosted CI only); 146 da-parse integration suites not in CI; ADR statuses mixed (042–049 Proposed); learning-critic-ruvllm outside workspace; CHANGELOG still describes gsd-pi hooks; doc staleness per DOCS-AUDIT (23 non-verified rows).

## Risks

- Static/graph verdicts may diverge from runtime behavior where CI does not execute (146 suites) — accepted as residual risk; every verdict carries a proof-weight tag, unverified stays explicit.
- Recorded run evidence ages: gate/preflight logs predate this milestone's doc edits; reports cite evidence as-of its recorded date.
- GROBID feasibility assessment (SC3d) estimates without running evals — feasibility notes are judgments, not measurements, and say so.

## Open questions

- [RESEARCH] Which of the retirement gates (SC3d list) require new measurement infrastructure versus only execution of existing packages — feeds the roadmap after M001.
- [RESEARCH] Whether the 146 non-CI da-parse suites should join CI before any promotion milestone — mapper finding, owner steers later.
- [NEEDS-USER] .codebase-memory/adr.md governance mirror regeneration path (generator script absent; accept-drift recorded for now — revisit if the mirror diverges materially).

## Corrections

- C1 2026-11-22: "ознакомится с репозиторием, изучить архитектуру, оценить реализованный функционал, оценить текущее состояние реализации grobid в репозитории" — owner overrode the proposed M521-carry goal; M001 is the orientation/assessment milestone, M521-carry postponed.
- C2 2026-11-22: "Held-out проверки живут в Verify-командах задач, не в коммитимых тест-файлах — E1 пере-диспозиционирована criterion SC1 (git-enumeration цитируемых путей отчёта в task Verify), E3 — criterion SC2 (grep словаря вердиктов в task Verify); N1/N3 .md-only доминирует" — owner selection verify-side at the plan checkpoint; Edge coverage rows E1/E3 updated accordingly; no committed check scripts in M001.
- C3 2026-11-22: "N1/E7 «только .md» действует на product-дифф (doc/review/**, аудированные .md); pipeline-owned состояние (.project/**, однострочный /build/ анкор в .gitignore) разрешено и не считается нарушением" — owner selection scope-product at the build checkpoint; the landing runtime requires committing .project/build/verify-ledger.jsonl state and the anchored /build/ rule; product code remains untouched (N1 substance preserved).
