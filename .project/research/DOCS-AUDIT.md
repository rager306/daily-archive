# Docs Audit

<!-- Written by the docs auditor (/skill:gsd-path-docs-audit or /skill:gsd-path-inspect).
     Every verdict carries evidence; a verdict without evidence is a defect. -->

Repo root: /root/daily-archive
Audited: 2026-11-22
Audited HEAD: 0f06b764c912f9c2d6025ff8427646c0cbaa7363
Alignment mode: no

## Summary

| Verdict | Count |
|---------|-------|
| verified | 21 |
| stale | 23 |
| aspirational | 0 |
| unverifiable | 1 |
| descriptive docs (no testable claims) | 1107 |

Worst drift: README.md and PRODUCT.md understate the shipped product by wide margins — the stage ledger is 19 stages not 14, the schema registry has 56 indexes not 26 and 43 node schemas not 31, the reverse-ADR audit has 65 rules not 7, and several README evidence artifacts (`artifacts/runtime/prompt_optimizer_readiness_m387.json`, `artifacts/gold_corpus/eval_aggregate_all_sources.json`, `artifacts/source_recovery/`) no longer exist at the cited paths.


## Doc: README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Binding ADRs: doc/adr/ADR-INDEX.md (037–042)" | structure | stale | README.md:10; doc/adr/ contains ADR-037…ADR-080 (43 ADR files) and ADR-INDEX.md lists through 080 — range stale |
| "pipeline_stage_contracts.yaml currently defines 14 stages" | structure | stale | README.md:72; data/ontology/pipeline_stage_contracts.yaml has 19 stages (acquisition…query; python3 yaml parse len(stages)=19) |
| "default summary generator paper-summary-generate-default → agnes-ai/agnes-2.0-flash via 9router" | config | stale | README.md:130-132; models.yaml contains neither binding id nor model name (grep across models.yaml: 0 hits); current bindings are summary-roles-v0.1/v0.2 tiers (models.yaml:118,54) |
| "figure analysis uses the figure-qa-primary models.yaml binding" | config | verified | README.md:141-142; models.yaml:102 binding_id: figure-qa-primary; data/ontology/prompt_tasks.yaml:103 model_binding_id: figure-qa-primary |
| "artifacts/runtime/prompt_optimizer_readiness_m387.json binds both prompt tasks" | status | stale | README.md:150; artifacts/runtime/ contains only vertical_scientific_path_m502.json — file absent |
| "Awesome Agentic Reasoning wave pinned at artifacts/source_recovery/awesome_agentic_reasoning_manifest.json (+assurance/.ids/.replay paths)" | status | stale | README.md:163-179; artifacts/source_recovery/ does not exist |
| "Current extraction metrics P=0.766 R=1.000 micro-F1=0.868 (see artifacts/gold_corpus/eval_aggregate_all_sources.json)" | status | stale | README.md:318-320; artifacts/gold_corpus/ contains only gold_corpus_manifest.json — cited aggregate absent from the cited path (an older copy survives at archive/artifacts-legacy/gold_corpus/eval_aggregate_all_sources.json) |
| "CLI commands table: schema-init, serve-graph, graph-ping, graph-stats, schema-list, ontology-export, heal, enrich, scheduler, query…" | command | verified | crates/da-cli/src/main.rs:345 (SchemaInit), 427 (SchemaList), 476 (OntologyExport), 842-850 (dispatch incl Heal); crates/da-cli/src/{serve_graph.rs,graph_ping.rs,graph_query.rs,audit_fields.rs,source_acquire.rs} exist |
| "kg-* universal crates Done (ADR-050): kg-ontology, kg-storage, kg-embeddings, kg-algorithms, kg-pipeline" | structure | verified | README.md:19; all five dirs exist under crates/ (ls crates) |
| "Layout: legacy/ frozen Python stub; source assets under data/article_catalog/article_catalog/arxiv/…" | structure | stale | README.md:9,475,171; neither legacy/ nor data/article_catalog/ exists in the tree at HEAD 0f06b764 |
| "Graph schema done — 31 node types, 26 indexes; `da schema-init` initializes all 26 graph indexes" | structure | stale | README.md:25,423; data/ontology/identity_indexes.yaml defines 46 and property_indexes.yaml 10 = 56 indexes loaded by schema_init (crates/da-cli/src/main.rs:2333), and data/ontology/node_schemas.yaml defines 43 node schemas — both counts stale |

## Doc: AGENTS.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| ".cargo/config.toml from .cargo/config.example.toml eliminates the rocksdb rebuild" | config | verified | AGENTS.md:241-243; .cargo/config.example.toml exists in tree |
| "governor CLI at scripts/governor/ with rules as DATA in checks/*.yaml" | structure | verified | AGENTS.md:276; scripts/governor/{__main__.py,domain,ports,adapters,application,checks} all exist; checks/ holds 41 entries |
| "Reverse-ADR audit enforces 7 rules: 4 artifacts-gate + 3 Rust-specific" | status | stale | AGENTS.md:285; scripts/governor/checks/adr_rules.yaml now defines 65 rules (python3 yaml: len(rules)=65) |
| "no_new_product_python_scripts (D237) allowlists scripts/governor/ and scripts/tests/" | structure | verified | AGENTS.md:291-294; both directories exist (ls scripts/governor, ls scripts/tests) |
| "Round-trip: da ontology-export → data/ontology/node_schemas.yaml → OntologyRegistry::load_from_dir → YamlSchemaAdapter" | command | verified | AGENTS.md:364; OntologyExport subcommand crates/da-cli/src/main.rs:476,935; data/ontology/node_schemas.yaml exists (43 node schemas) |
| ".project/ is committed; gsd-path is the active engine; user work in stash@{0}" | status | stale | AGENTS.md:224-225; at audited HEAD 0f06b764 no .project/ path exists in the tree and `git ls-files .project` is empty — the claim postdates the recorded baseline |

## Doc: PRODUCT.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "14-stage pipeline ledger (data/ontology/pipeline_stage_contracts.yaml)" | structure | stale | PRODUCT.md:14; the ledger defines 19 stages (yaml parse) |
| "schema registry and validators (26 indexes, edge contracts, node schemas)" | structure | stale | PRODUCT.md:16; data/ontology/identity_indexes.yaml defines 46 and property_indexes.yaml 10 indexes = 56, not 26 (schema_init loads both, crates/da-cli/src/main.rs:2333) |
| "ontology export (31 schemas → YAML)" | structure | stale | PRODUCT.md:16; data/ontology/node_schemas.yaml defines 43 node schemas |
| "gold-corpus evaluation micro-F1 0.869; committed 2026-08-09 baseline 0.8677 at archive/artifacts-legacy/gold_corpus/eval_aggregate_all_sources.json" | status | verified | PRODUCT.md:12; the cited archive file exists (ls archive/artifacts-legacy/gold_corpus/eval_aggregate_all_sources.json); number values match the doc's own quoted measurement record |
| "Deterministic RuleBasedExtractor over parsed TEI" | feature | verified | PRODUCT.md:12; crates/da-adapters/src/rule_extractor.rs (+ lib.rs, pipeline_stages.rs references) |
| "Catalog sweep materialized 589 papers into a committed .sgsnap" | status | verified | PRODUCT.md:14; data/graphs/catalog_materialized.sgsnap exists (ls data/graphs); count 589 itself not re-derived |
| "PDF via Java GROBID (HTTP :8070), profiles structural/rich" | integration | verified | PRODUCT.md:11; .env.example:104-119 (GROBID_URL=http://127.0.0.1:8070), crates/da-adapters/src/grobid_parser.rs exists |

## Doc: OKF.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Tree layout includes logs/ (structured logs, ADR-052)" | structure | stale | OKF.md:12; logs/ does not exist in the tree |
| "Tree layout: data/, governor/, crates/, scripts/governor/, doc/, artifacts/, archive/, target/" | structure | verified | OKF.md:5-13; all named dirs exist (ls) |
| "Unified CLI: PYTHONPATH=scripts python3 -m governor --help; full gate: governor architecture" | command | verified | OKF.md:15-16; scripts/governor/__main__.py exists; AGENTS.md:265 documents `governor architecture` gate (command surface present) |

## Doc: CONCEPTS.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Import lock surfaced as import_eligible=false (decision D127)" | feature | verified | CONCEPTS.md:8-10; data/ontology/pipeline_stage_contracts.yaml:2 `import_eligible: false`; crates/da-cli/src/main.rs:107 prints D127 locked default |

## Doc: WORKFLOW.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "The gsd-path router reads .project/STATE.md and invokes the next skill" | feature | stale | WORKFLOW.md:2-4; .project/ does not exist at audited HEAD 0f06b764, so the on-disk state the router reads is absent at this revision |

## Doc: crates/kg-storage/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Provides GraphStore/DirectGraphStore traits, MockGraphStore, SamyamaGraphStore adapter, port contract suite (ADR-051 Contour 4)" | structure | verified | crates/kg-storage/src/{traits.rs,mock.rs,samyama_adapter.rs,contract.rs} all exist |

## Doc: crates/kg-ontology/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Phase A (current): skeleton crate with Severity, Violation, PropertySnapshot, format_violations" | status | stale | crates/kg-ontology/README.md:9-14; the crate has grown to ~13 modules (vid.rs, temporal.rs, registry.rs, healing.rs, edge_contract.rs, schema.rs, retry.rs, bitemporal.rs…) with ~86 #[test] fns — no longer a Phase A skeleton |

## Doc: crates/kg-pipeline/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "PipelineBuilder, ExecutionEngine, FailurePolicy, RetryPolicy exist" | structure | verified | crates/kg-pipeline/src/{builder.rs,engine.rs,failure.rs,retry.rs} exist |
| "Phase E: 7 tests" | status | stale | crates/kg-pipeline/README.md:13; static count of #[test] in src is now ≥11 (builder.rs 5, failure.rs 6) |

## Doc: crates/kg-algorithms/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Phase E (current): temporal edge resolution (4-rule ADR-047); 9 tests" | status | stale | crates/kg-algorithms/README.md; crate now also carries csr.rs, ppr.rs, view_algorithms.rs, port.rs with 33 #[test] fns total (temporal.rs 15, csr.rs 11, ppr.rs 6, port.rs 1) |

## Doc: doc/GRAPH-SCHEMA.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Authoritative source of node schemas is da_domain::schema::all_node_schemas(); this summary is rendered by da schema-list" | structure | verified | doc/GRAPH-SCHEMA.md:18-21; crates/da-domain/src/schema.rs:15 `pub fn all_node_schemas()`; SchemaList subcommand crates/da-cli/src/main.rs:427 |

## Doc: doc/adr/ADR-INDEX.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Index lists ADR-037…ADR-080 with statuses and dates" | structure | verified | doc/adr/ holds 45 files = 43 ADRs numbered 037–080 plus ADR-INDEX.md; index table matches (head -40 ADR-INDEX.md) |
| "Python-era ADRs (ADR-001..036) … see legacy/doc/adr/" | structure | stale | doc/adr/ADR-INDEX.md:3-4; legacy/ does not exist in the tree at audited HEAD |

## Doc: doc/OKF.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "doc/adr/ ← Architecture Decision Records (ADR-001…067)" | structure | stale | doc/OKF.md:4; current ADRs span 037–080 (43 files) |

## Doc: scripts/OKF.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "scripts/governor/ is a hexagonal Python package (domain/ports/adapters/application/checks/*.yaml/tests)" | structure | verified | scripts/OKF.md:3-10; all named subpackages exist under scripts/governor/ |
| "scripts/check_project_trajectory.py legacy entry point (→ governor)" | structure | stale | scripts/OKF.md:11; scripts/check_project_trajectory.py does not exist (ls scripts shows no such file) |

## Doc: governor/OKF.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "governor/fixtures/ holds positive/negative contract pairs incl acap_runtime_control_plane, equation_coverage_contract" | structure | verified | governor/OKF.md:3-11; governor/fixtures/ contains acap_runtime_control_plane, equation_coverage_contract and more (ls governor/fixtures) |

## Doc: .docker/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "GROBID TEI consumed by Rust adapter da_adapters::GrobidParser at crates/da-adapters/src/grobid_parser.rs" | integration | verified | .docker/README.md:3-4; crates/da-adapters/src/grobid_parser.rs exists |
| "Python research_graph.* paths frozen under legacy/, not on runtime path" | status | stale | .docker/README.md:5; legacy/ absent from the tree at audited HEAD |

## Doc: tools/gsd-engine-patches/README.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Patches applied to the installed @opengsd/gsd-pi 1.21.0 outside this repo, keeping *.pre-projection-lock-patch backups" | config | unverifiable | tools/gsd-engine-patches/README.md:3-8; the patched engine lives outside the repository (installed npm package), so no in-repo evidence can confirm the patches or backups — would need the machine's installed gsd-pi path |

## Doc: .codebase-memory/adr.md

| Claim | Type | Verdict | Evidence |
|-------|------|---------|----------|
| "Generated by scripts/sync_codebase_memory_governance.py" | status | stale | .codebase-memory/adr.md:3; scripts/sync_codebase_memory_governance.py does not exist — the generator behind this mirror is gone |

### Scope reasons

- AGENTS.md gsd-path source-layout section (`skills/gsd-path/`, `scripts/sync_skill_resources.py`, `platforms/shared-agents/dispatch.md`): the doc itself scopes these to the GSD Path source checkout and states an installed application need not contain them; this repository is a consuming application, so these are dependency claims, not product claims — not audited as findings.
- WORKFLOW.md program-flow narrative describes the installed gsd-path skill's pipeline behavior (dependency), kept out of claim rows except its on-disk `.project/STATE.md` touchpoint.
- WORKFLOW.md makes no further product claims testable against this repo at HEAD.
- .codebase-memory/adr.md: only its generator-path claim is testable; the mirrored ADR content is a copy of canonical ADRs audited via doc/adr/ADR-INDEX.md.
- doc/review/README.md: a dated (2026-09-27) corrective-wave status snapshot recording completed measurement work; its claims are historical results, not statements about current product behavior.

## Descriptive docs

<!-- Every inventoried doc with no testable claims; union with Doc sections above equals the frozen inventory (1125 files). -->

- .agents/skills/async-python-patterns/SKILL.md
- .agents/skills/code-review-excellence/SKILL.md
- .agents/skills/docx/SKILL.md
- .agents/skills/error-handling-patterns/SKILL.md
- .agents/skills/git-advanced-workflows/SKILL.md
- .agents/skills/glm-zai-safe-helper/SKILL.md
- .agents/skills/glm-zai-safe-helper/references/zai-findings.md
- .agents/skills/ladybugdb/SKILL.md
- .agents/skills/pdf/SKILL.md
- .agents/skills/pdf/forms.md
- .agents/skills/pdf/reference.md
- .agents/skills/pptx/SKILL.md
- .agents/skills/pptx/editing.md
- .agents/skills/pptx/pptxgenjs.md
- .agents/skills/python-performance-optimization/SKILL.md
- .agents/skills/python-performance-optimization/references/advanced-patterns.md
- .agents/skills/python-testing-patterns/SKILL.md
- .agents/skills/python-testing-patterns/references/advanced-patterns.md
- .agents/skills/skill-creator/SKILL.md
- .agents/skills/skill-creator/agents/analyzer.md
- .agents/skills/skill-creator/agents/comparator.md
- .agents/skills/skill-creator/agents/grader.md
- .agents/skills/skill-creator/references/schemas.md
- .agents/skills/uv-package-manager/SKILL.md
- .agents/skills/uv-package-manager/references/advanced-patterns.md
- .agents/skills/xlsx/SKILL.md
- .commandcode/taste/taste.md
- .compound-engineering/STRATEGY.md
- .compound-engineering/artifacts/plans/2026-08-26-1515-refactor-da-parse-crate-plan.md
- .compound-engineering/artifacts/plans/2026-09-22-1746-feature-m511-grobid-semantic-correctness-plan.md
- .compound-engineering/artifacts/research/2026-08-26-scholarly-parse-mcp-grounding.md
- .compound-engineering/artifacts/research/2026-08-27-da-parse-elegance-vendor-gap-assessment.md
- .compound-engineering/artifacts/research/2026-08-27-grobid-rust-ruvector-platform-research.md
- AGENTS.pre-path.md
- CHANGELOG.md
- PLAN-parallel-experiment.md
- STRATEGY.md
- VISION.md
- archive/OKF.md
- archive/PYTHON-ERA-COLD.md
- archive/artifacts-legacy/README.md
- archive/artifacts-legacy/article_context/paper_2602.04837_inventory/REPORT.md
- archive/artifacts-legacy/article_context/paper_2608.07067_inventory/REPORT.md
- archive/artifacts-legacy/catalog_sweep/analysis.md
- archive/artifacts-legacy/d127_unlock/REPORT.md
- archive/artifacts-legacy/gepa_hub_production/REPORT.md
- archive/artifacts-legacy/gepa_hub_research/REPORT.md
- archive/artifacts-legacy/loads/quote_survey/SURVEY.md
- archive/artifacts-legacy/m388/s05_hotpath_seam_audit.md
- archive/artifacts-legacy/m400-s05-multimodal-closeout.md
- archive/data-legacy/architecture-assessment/m163-async-threading.md
- archive/data-legacy/architecture-assessment/m163-code-layering.md
- archive/data-legacy/architecture-assessment/m163-decisions.md
- archive/data-legacy/architecture-assessment/m163-final-assessment.md
- archive/data-legacy/architecture-assessment/m163-test-architecture.md
- archive/data-legacy/architecture-assessment/m164-closeout.md
- archive/data-legacy/architecture-assessment/m164-contract-classification.md
- archive/data-legacy/architecture-assessment/m164-strict-boundary-baseline.md
- archive/data-legacy/architecture-assessment/m165-async-thread-readiness-assessment.md
- archive/data-legacy/architecture-assessment/m165-code-layering-assessment.md
- archive/data-legacy/architecture-assessment/m165-current-baseline.md
- archive/data-legacy/architecture-assessment/m165-decision-doc-assessment.md
- archive/data-legacy/architecture-assessment/m165-final-strict-architecture-assessment.md
- archive/data-legacy/architecture-assessment/m165-test-architecture-assessment.md
- archive/data-legacy/architecture-assessment/m166-p1-remediation-baseline.md
- archive/data-legacy/architecture-assessment/m166-p1-remediation-closeout.md
- archive/data-legacy/architecture-assessment/m167-hardening-baseline.md
- archive/data-legacy/architecture-assessment/m167-hardening-closeout.md
- archive/data-legacy/architecture-assessment/m167-queue-concurrency-recon.md
- archive/data-legacy/architecture-assessment/m167-queue-concurrency-result.md
- archive/data-legacy/architecture-assessment/m167-test-allowlist-recon.md
- archive/data-legacy/architecture-assessment/m167-test-allowlist-result.md
- archive/data-legacy/architecture-assessment/m167-write-path-classification.md
- archive/data-legacy/architecture-assessment/m167-write-path-inventory.md
- archive/data-legacy/architecture-assessment/m168-catalog-write-recon.md
- archive/data-legacy/architecture-assessment/m168-catalog-write-result.md
- archive/data-legacy/architecture-assessment/m168-dynamic-test-batch-one.md
- archive/data-legacy/architecture-assessment/m168-dynamic-test-final.md
- archive/data-legacy/architecture-assessment/m168-dynamic-test-recon.md
- archive/data-legacy/architecture-assessment/m168-hardening-baseline.md
- archive/data-legacy/architecture-assessment/m168-hardening-closeout.md
- archive/data-legacy/architecture-assessment/m168-integrated-verification.md
- archive/data-legacy/architecture-assessment/m168-queue-stress-recon.md
- archive/data-legacy/architecture-assessment/m168-queue-stress-result.md
- archive/data-legacy/architecture-assessment/m168-write-path-inventory.md
- archive/data-legacy/architecture-assessment/m168-write-path-unknown-reduction.md
- archive/data-legacy/architecture-assessment/m169-baseline.md
- archive/data-legacy/architecture-assessment/m169-cli-write-resolution.md
- archive/data-legacy/architecture-assessment/m169-feasibility.md
- archive/data-legacy/architecture-assessment/m169-fetcher-write-resolution.md
- archive/data-legacy/architecture-assessment/m169-hardening-closeout.md
- archive/data-legacy/architecture-assessment/m169-integrated-verification.md
- archive/data-legacy/architecture-assessment/m169-m061-artifact-recon.md
- archive/data-legacy/architecture-assessment/m169-m061-reconciliation-harness.md
- archive/data-legacy/architecture-assessment/m169-m061-reconciliation-result.md
- archive/data-legacy/architecture-assessment/m169-queue-soak-design.md
- archive/data-legacy/architecture-assessment/m169-queue-soak-result.md
- archive/data-legacy/architecture-assessment/m169-write-path-inventory-closeout.md
- archive/data-legacy/architecture-assessment/m169-write-path-inventory.md
- archive/data-legacy/architecture-assessment/m169-write-path-ownership-recon.md
- archive/data-legacy/architecture-assessment/m170-acceptance-contract.md
- archive/data-legacy/architecture-assessment/m170-architecture-backlog-inventory.md
- archive/data-legacy/architecture-assessment/m170-architecture-ratchet-update.md
- archive/data-legacy/architecture-assessment/m170-architecture-remediation-readiness.md
- archive/data-legacy/architecture-assessment/m170-architecture-remediation-shared-state.md
- archive/data-legacy/architecture-assessment/m170-baseline.md
- archive/data-legacy/architecture-assessment/m170-cache-coordination-policy.md
- archive/data-legacy/architecture-assessment/m170-cache-coordination-verification.md
- archive/data-legacy/architecture-assessment/m170-cli-cache-coordination-result.md
- archive/data-legacy/architecture-assessment/m170-feasibility.md
- archive/data-legacy/architecture-assessment/m170-hardening-closeout.md
- archive/data-legacy/architecture-assessment/m170-integrated-verification.md
- archive/data-legacy/architecture-assessment/m170-pdf-cache-coordination-result.md
- archive/data-legacy/architecture-assessment/m170-quality-stack.md
- archive/data-legacy/architecture-assessment/m170-queue-soak-contract.md
- archive/data-legacy/architecture-assessment/m170-queue-soak-result.md
- archive/data-legacy/architecture-assessment/m170-shared-state-review.md
- archive/data-legacy/architecture-assessment/m170-write-path-inventory-cache-check.md
- archive/data-legacy/architecture-assessment/m170-write-path-inventory-integrated.md
- archive/data-legacy/architecture-assessment/m170-write-path-inventory-ratchet.md
- archive/data-legacy/architecture-assessment/m170-write-path-inventory.md
- archive/data-legacy/architecture-assessment/m171-activation-readiness.md
- archive/data-legacy/architecture-assessment/m171-activation-scope-contract.md
- archive/data-legacy/architecture-assessment/m171-baseline.md
- archive/data-legacy/architecture-assessment/m171-environment-soak-contract.md
- archive/data-legacy/architecture-assessment/m171-environment-soak-result.md
- archive/data-legacy/architecture-assessment/m171-environment-soak-runner.md
- archive/data-legacy/architecture-assessment/m171-feasibility.md
- archive/data-legacy/architecture-assessment/m171-hardening-closeout.md
- archive/data-legacy/architecture-assessment/m171-integrated-verification.md
- archive/data-legacy/architecture-assessment/m171-inventory-category-closeout.md
- archive/data-legacy/architecture-assessment/m171-inventory-category-recon.md
- archive/data-legacy/architecture-assessment/m171-inventory-impact-analysis.md
- archive/data-legacy/architecture-assessment/m171-quality-stack.md
- archive/data-legacy/architecture-assessment/m171-queue-activation-checklist.md
- archive/data-legacy/architecture-assessment/m171-queue-activation-recon.md
- archive/data-legacy/architecture-assessment/m171-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m171-write-path-inventory-categorized.md
- archive/data-legacy/architecture-assessment/m171-write-path-inventory-integrated.md
- archive/data-legacy/architecture-assessment/m172-baseline.md
- archive/data-legacy/architecture-assessment/m172-category-candidates.md
- archive/data-legacy/architecture-assessment/m172-category-scope-decision.md
- archive/data-legacy/architecture-assessment/m172-feasibility.md
- archive/data-legacy/architecture-assessment/m172-impact-analysis.md
- archive/data-legacy/architecture-assessment/m172-integrated-verification.md
- archive/data-legacy/architecture-assessment/m172-inventory-category-expansion-closeout.md
- archive/data-legacy/architecture-assessment/m172-inventory-delta.md
- archive/data-legacy/architecture-assessment/m172-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m172-quality-stack.md
- archive/data-legacy/architecture-assessment/m172-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m172-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m173-baseline.md
- archive/data-legacy/architecture-assessment/m173-category-candidates.md
- archive/data-legacy/architecture-assessment/m173-category-scope-decision.md
- archive/data-legacy/architecture-assessment/m173-feasibility.md
- archive/data-legacy/architecture-assessment/m173-impact-analysis.md
- archive/data-legacy/architecture-assessment/m173-integrated-verification.md
- archive/data-legacy/architecture-assessment/m173-inventory-category-batch-two-closeout.md
- archive/data-legacy/architecture-assessment/m173-inventory-delta.md
- archive/data-legacy/architecture-assessment/m173-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m173-quality-stack.md
- archive/data-legacy/architecture-assessment/m173-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m173-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m174-baseline.md
- archive/data-legacy/architecture-assessment/m174-feasibility.md
- archive/data-legacy/architecture-assessment/m174-impact-analysis.md
- archive/data-legacy/architecture-assessment/m174-integrated-verification.md
- archive/data-legacy/architecture-assessment/m174-inventory-delta.md
- archive/data-legacy/architecture-assessment/m174-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m174-quality-stack.md
- archive/data-legacy/architecture-assessment/m174-repair-benchmark-category-closeout.md
- archive/data-legacy/architecture-assessment/m174-repair-category-candidates.md
- archive/data-legacy/architecture-assessment/m174-repair-category-scope-decision.md
- archive/data-legacy/architecture-assessment/m174-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m174-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m175-baseline.md
- archive/data-legacy/architecture-assessment/m175-broad-output-review.md
- archive/data-legacy/architecture-assessment/m175-cli-broad-delta-closeout.md
- archive/data-legacy/architecture-assessment/m175-command-output-candidates.md
- archive/data-legacy/architecture-assessment/m175-delta-reporting-recon.md
- archive/data-legacy/architecture-assessment/m175-feasibility.md
- archive/data-legacy/architecture-assessment/m175-impact-analysis.md
- archive/data-legacy/architecture-assessment/m175-integrated-verification.md
- archive/data-legacy/architecture-assessment/m175-inventory-delta.md
- archive/data-legacy/architecture-assessment/m175-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m175-quality-stack.md
- archive/data-legacy/architecture-assessment/m175-scope-decision.md
- archive/data-legacy/architecture-assessment/m175-test-plan.md
- archive/data-legacy/architecture-assessment/m175-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m175-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m176-baseline.md
- archive/data-legacy/architecture-assessment/m176-impact-analysis.md
- archive/data-legacy/architecture-assessment/m176-integrated-verification.md
- archive/data-legacy/architecture-assessment/m176-inventory-delta.md
- archive/data-legacy/architecture-assessment/m176-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m176-quality-stack.md
- archive/data-legacy/architecture-assessment/m176-script-candidates.md
- archive/data-legacy/architecture-assessment/m176-script-only-wave-one-closeout.md
- archive/data-legacy/architecture-assessment/m176-script-scope-decision.md
- archive/data-legacy/architecture-assessment/m176-test-plan.md
- archive/data-legacy/architecture-assessment/m176-wave-bounds.md
- archive/data-legacy/architecture-assessment/m176-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m176-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m177-baseline.md
- archive/data-legacy/architecture-assessment/m177-cache-policy-review.md
- archive/data-legacy/architecture-assessment/m177-ci-delta-wiring.md
- archive/data-legacy/architecture-assessment/m177-combined-candidates.md
- archive/data-legacy/architecture-assessment/m177-combined-feasibility.md
- archive/data-legacy/architecture-assessment/m177-combined-ownership-wave-closeout.md
- archive/data-legacy/architecture-assessment/m177-impact-analysis.md
- archive/data-legacy/architecture-assessment/m177-integrated-verification.md
- archive/data-legacy/architecture-assessment/m177-inventory-delta.md
- archive/data-legacy/architecture-assessment/m177-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m177-quality-stack.md
- archive/data-legacy/architecture-assessment/m177-queue-smoke-ownership.md
- archive/data-legacy/architecture-assessment/m177-scope-decision.md
- archive/data-legacy/architecture-assessment/m177-test-plan.md
- archive/data-legacy/architecture-assessment/m177-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m177-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m178-baseline.md
- archive/data-legacy/architecture-assessment/m178-cache-coordination-review.md
- archive/data-legacy/architecture-assessment/m178-ci-drift-final-recheck.md
- archive/data-legacy/architecture-assessment/m178-ci-drift-policy.md
- archive/data-legacy/architecture-assessment/m178-impact-analysis.md
- archive/data-legacy/architecture-assessment/m178-integrated-verification.md
- archive/data-legacy/architecture-assessment/m178-inventory-delta.md
- archive/data-legacy/architecture-assessment/m178-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m178-quality-stack.md
- archive/data-legacy/architecture-assessment/m178-residual-script-candidates.md
- archive/data-legacy/architecture-assessment/m178-residual-script-drift-cache-closeout.md
- archive/data-legacy/architecture-assessment/m178-scope-decision.md
- archive/data-legacy/architecture-assessment/m178-script-wave-result.md
- archive/data-legacy/architecture-assessment/m178-script-wave-scope.md
- archive/data-legacy/architecture-assessment/m178-test-plan.md
- archive/data-legacy/architecture-assessment/m178-three-direction-feasibility.md
- archive/data-legacy/architecture-assessment/m178-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m178-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m179-baseline.md
- archive/data-legacy/architecture-assessment/m179-cache-lifecycle-candidates.md
- archive/data-legacy/architecture-assessment/m179-cache-lifecycle-review.md
- archive/data-legacy/architecture-assessment/m179-canonical-baseline-final-check.md
- archive/data-legacy/architecture-assessment/m179-canonical-baseline-policy.md
- archive/data-legacy/architecture-assessment/m179-impact-analysis.md
- archive/data-legacy/architecture-assessment/m179-integrated-verification.md
- archive/data-legacy/architecture-assessment/m179-inventory-delta.md
- archive/data-legacy/architecture-assessment/m179-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m179-quality-stack.md
- archive/data-legacy/architecture-assessment/m179-residual-canonical-cache-closeout.md
- archive/data-legacy/architecture-assessment/m179-residual-script-candidates.md
- archive/data-legacy/architecture-assessment/m179-scope-decision.md
- archive/data-legacy/architecture-assessment/m179-script-wave-delta.md
- archive/data-legacy/architecture-assessment/m179-script-wave-result.md
- archive/data-legacy/architecture-assessment/m179-script-wave-scope.md
- archive/data-legacy/architecture-assessment/m179-test-plan.md
- archive/data-legacy/architecture-assessment/m179-three-direction-feasibility.md
- archive/data-legacy/architecture-assessment/m179-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m179-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m179-write-path-inventory-interim.md
- archive/data-legacy/architecture-assessment/m180-baseline.md
- archive/data-legacy/architecture-assessment/m180-cache-lifecycle-candidates.md
- archive/data-legacy/architecture-assessment/m180-cache-lifecycle-review.md
- archive/data-legacy/architecture-assessment/m180-canonical-baseline-final-check.md
- archive/data-legacy/architecture-assessment/m180-canonical-soak-policy.md
- archive/data-legacy/architecture-assessment/m180-impact-analysis.md
- archive/data-legacy/architecture-assessment/m180-integrated-verification.md
- archive/data-legacy/architecture-assessment/m180-inventory-delta.md
- archive/data-legacy/architecture-assessment/m180-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m180-quality-stack.md
- archive/data-legacy/architecture-assessment/m180-residual-verify-candidates.md
- archive/data-legacy/architecture-assessment/m180-scope-decision.md
- archive/data-legacy/architecture-assessment/m180-test-plan.md
- archive/data-legacy/architecture-assessment/m180-three-direction-feasibility.md
- archive/data-legacy/architecture-assessment/m180-verify-canonical-cache-closeout.md
- archive/data-legacy/architecture-assessment/m180-verify-wave-delta.md
- archive/data-legacy/architecture-assessment/m180-verify-wave-result.md
- archive/data-legacy/architecture-assessment/m180-verify-wave-scope.md
- archive/data-legacy/architecture-assessment/m180-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m180-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m180-write-path-inventory-interim.md
- archive/data-legacy/architecture-assessment/m181-baseline.md
- archive/data-legacy/architecture-assessment/m181-cache-no-move-result.md
- archive/data-legacy/architecture-assessment/m181-cache-ownership-candidates.md
- archive/data-legacy/architecture-assessment/m181-cache-ownership-review.md
- archive/data-legacy/architecture-assessment/m181-canonical-baseline-final-check.md
- archive/data-legacy/architecture-assessment/m181-canonical-docs-ci-reference-recon.md
- archive/data-legacy/architecture-assessment/m181-canonical-docs-cleanup-result.md
- archive/data-legacy/architecture-assessment/m181-canonical-docs-cleanup-scope.md
- archive/data-legacy/architecture-assessment/m181-combined-feasibility.md
- archive/data-legacy/architecture-assessment/m181-impact-analysis.md
- archive/data-legacy/architecture-assessment/m181-integrated-verification.md
- archive/data-legacy/architecture-assessment/m181-inventory-delta.md
- archive/data-legacy/architecture-assessment/m181-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m181-quality-stack.md
- archive/data-legacy/architecture-assessment/m181-residual-verify-candidates.md
- archive/data-legacy/architecture-assessment/m181-test-plan.md
- archive/data-legacy/architecture-assessment/m181-verify-docs-cache-closeout.md
- archive/data-legacy/architecture-assessment/m181-verify-wave-delta.md
- archive/data-legacy/architecture-assessment/m181-verify-wave-result.md
- archive/data-legacy/architecture-assessment/m181-verify-wave-scope.md
- archive/data-legacy/architecture-assessment/m181-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m181-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m181-write-path-inventory-interim.md
- archive/data-legacy/architecture-assessment/m182-baseline.md
- archive/data-legacy/architecture-assessment/m182-build-replay-closeout.md
- archive/data-legacy/architecture-assessment/m182-canonical-baseline-final-check.md
- archive/data-legacy/architecture-assessment/m182-impact-analysis.md
- archive/data-legacy/architecture-assessment/m182-integrated-verification.md
- archive/data-legacy/architecture-assessment/m182-inventory-delta.md
- archive/data-legacy/architecture-assessment/m182-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m182-quality-stack.md
- archive/data-legacy/architecture-assessment/m182-test-plan.md
- archive/data-legacy/architecture-assessment/m182-wave-delta.md
- archive/data-legacy/architecture-assessment/m182-wave-result.md
- archive/data-legacy/architecture-assessment/m182-wave-scope.md
- archive/data-legacy/architecture-assessment/m182-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m182-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m182-write-path-inventory-interim.md
- archive/data-legacy/architecture-assessment/m183-architecture-crystallization-closeout.md
- archive/data-legacy/architecture-assessment/m183-baseline.md
- archive/data-legacy/architecture-assessment/m183-cache-lifecycle-candidates.md
- archive/data-legacy/architecture-assessment/m183-cache-lifecycle-review.md
- archive/data-legacy/architecture-assessment/m183-cache-no-move-result.md
- archive/data-legacy/architecture-assessment/m183-candidate-feasibility.md
- archive/data-legacy/architecture-assessment/m183-canonical-baseline-final-check.md
- archive/data-legacy/architecture-assessment/m183-docs-crystallization-recon.md
- archive/data-legacy/architecture-assessment/m183-docs-crystallization-result.md
- archive/data-legacy/architecture-assessment/m183-impact-analysis.md
- archive/data-legacy/architecture-assessment/m183-integrated-verification.md
- archive/data-legacy/architecture-assessment/m183-inventory-delta.md
- archive/data-legacy/architecture-assessment/m183-inventory-final-check.md
- archive/data-legacy/architecture-assessment/m183-quality-stack.md
- archive/data-legacy/architecture-assessment/m183-test-plan.md
- archive/data-legacy/architecture-assessment/m183-wave-delta.md
- archive/data-legacy/architecture-assessment/m183-wave-result.md
- archive/data-legacy/architecture-assessment/m183-wave-scope.md
- archive/data-legacy/architecture-assessment/m183-write-path-inventory-baseline.md
- archive/data-legacy/architecture-assessment/m183-write-path-inventory-final.md
- archive/data-legacy/architecture-assessment/m183-write-path-inventory-interim.md
- archive/data-legacy/architecture-assessment/m184-acquisition-source-audit.md
- archive/data-legacy/architecture-assessment/m184-acquisition-source-delta.md
- archive/data-legacy/architecture-assessment/m184-acquisition-source-result.md
- archive/data-legacy/architecture-assessment/m184-architecture-refactor-closeout.md
- archive/data-legacy/architecture-assessment/m184-architecture-state.md
- archive/data-legacy/architecture-assessment/m184-audit-analysis-audit.md
- archive/data-legacy/architecture-assessment/m184-audit-analysis-delta.md
- archive/data-legacy/architecture-assessment/m184-audit-analysis-result.md
- archive/data-legacy/architecture-assessment/m184-baseline.md
- archive/data-legacy/architecture-assessment/m184-cache-manifest-proof-gate.md
- archive/data-legacy/architecture-assessment/m184-cache-manifest-result.md
- archive/data-legacy/architecture-assessment/m184-cache-manifest-review.md
- archive/data-legacy/architecture-assessment/m184-gitnexus-planning.md
- archive/data-legacy/architecture-assessment/m184-graph-connectivity-audit.md
- archive/data-legacy/architecture-assessment/m184-graph-connectivity-delta.md
- archive/data-legacy/architecture-assessment/m184-graph-connectivity-result.md
- archive/data-legacy/architecture-assessment/m184-integrated-verification.md
- archive/data-legacy/architecture-assessment/m184-long-horizon-map.md
- archive/data-legacy/architecture-assessment/m184-quality-stack.md
- archive/data-legacy/architecture-assessment/m184-ratchet-ownership-contract.md
- archive/data-legacy/architecture-assessment/m184-ratchet-result.md
- archive/data-legacy/architecture-assessment/m184-remaining-residual-delta.md
- archive/data-legacy/architecture-assessment/m184-remaining-residual-result.md
- archive/data-legacy/architecture-assessment/m184-remaining-residual-triage.md
- archive/data-legacy/architecture-assessment/m184-render-report-audit.md
- archive/data-legacy/architecture-assessment/m184-render-report-delta.md
- archive/data-legacy/architecture-assessment/m184-render-report-result.md
- archive/data-legacy/architecture-assessment/m184-replay-conversion-audit.md
- archive/data-legacy/architecture-assessment/m184-replay-conversion-delta.md
- archive/data-legacy/architecture-assessment/m184-replay-conversion-result.md
- archive/data-legacy/architecture-assessment/m184-s03-after.md
- archive/data-legacy/architecture-assessment/m184-s03-baseline.md
- archive/data-legacy/architecture-assessment/m184-s04-after.md
- archive/data-legacy/architecture-assessment/m184-s04-baseline.md
- archive/data-legacy/architecture-assessment/m184-s05-after.md
- archive/data-legacy/architecture-assessment/m184-s05-baseline.md
- archive/data-legacy/architecture-assessment/m184-s06-after.md
- archive/data-legacy/architecture-assessment/m184-s06-baseline.md
- archive/data-legacy/architecture-assessment/m184-s07-after.md
- archive/data-legacy/architecture-assessment/m184-s07-baseline.md
- archive/data-legacy/architecture-assessment/m184-s08-after.md
- archive/data-legacy/architecture-assessment/m184-s08-baseline.md
- archive/data-legacy/architecture-assessment/m184-test-contract-alignment-result.md
- archive/data-legacy/architecture-assessment/m184-test-contract-alignment.md
- archive/data-legacy/architecture-assessment/m184-wrapper-extraction-candidate.md
- archive/data-legacy/architecture-assessment/m184-wrapper-extraction-result.md
- archive/data-legacy/architecture-assessment/m185-architecture-guard-verification.md
- archive/data-legacy/architecture-assessment/m185-architecture-horizon-two-closeout.md
- archive/data-legacy/architecture-assessment/m185-architecture-horizon-two-state.md
- archive/data-legacy/architecture-assessment/m185-candidate-inventory.md
- archive/data-legacy/architecture-assessment/m185-execution-map.md
- archive/data-legacy/architecture-assessment/m185-gitnexus-refresh.md
- archive/data-legacy/architecture-assessment/m185-gitnexus-status-review.md
- archive/data-legacy/architecture-assessment/m185-integrated-focused-tests.md
- archive/data-legacy/architecture-assessment/m185-m025-verifier-boundary-assessment.md
- archive/data-legacy/architecture-assessment/m185-m025-verifier-boundary-result.md
- archive/data-legacy/architecture-assessment/m185-m025-verifier-boundary-verification.md
- archive/data-legacy/architecture-assessment/m185-m055-manifest-lifecycle-assessment.md
- archive/data-legacy/architecture-assessment/m185-m055-manifest-lifecycle-result.md
- archive/data-legacy/architecture-assessment/m185-m055-manifest-lifecycle-verification.md
- archive/data-legacy/architecture-assessment/m185-m058-graph-manifest-lifecycle-assessment.md
- archive/data-legacy/architecture-assessment/m185-m058-graph-manifest-lifecycle-result.md
- archive/data-legacy/architecture-assessment/m185-m058-graph-manifest-lifecycle-verification.md
- archive/data-legacy/architecture-assessment/m185-m059-manifest-lifecycle-assessment.md
- archive/data-legacy/architecture-assessment/m185-m059-manifest-lifecycle-result.md
- archive/data-legacy/architecture-assessment/m185-m059-manifest-lifecycle-verification.md
- archive/data-legacy/architecture-assessment/m185-manifest-residual-decision-gate.md
- archive/data-legacy/architecture-assessment/m185-manifest-residual-gate-verification.md
- archive/data-legacy/architecture-assessment/m185-manifest-residual-inventory-check.md
- archive/data-legacy/architecture-assessment/m185-pipeline-audit-extraction-assessment.md
- archive/data-legacy/architecture-assessment/m185-pipeline-audit-extraction-result.md
- archive/data-legacy/architecture-assessment/m185-quality-stack.md
- archive/data-legacy/architecture-assessment/m185-s12-verification.md
- archive/data-legacy/architecture-assessment/m185-s13-verification.md
- archive/data-legacy/architecture-assessment/m185-s14-verification.md
- archive/data-legacy/architecture-assessment/m185-test-architecture-extraction-assessment.md
- archive/data-legacy/architecture-assessment/m185-test-architecture-extraction-result.md
- archive/data-legacy/architecture-assessment/m185-validation-evidence-helper-assessment.md
- archive/data-legacy/architecture-assessment/m185-validation-evidence-helper-result.md
- archive/data-legacy/architecture-assessment/m185-validation-evidence-helper-verification.md
- archive/data-legacy/architecture-assessment/m185-wrapper-contract-baseline.md
- archive/data-legacy/architecture-assessment/m185-wrapper-contract-result.md
- archive/data-legacy/architecture-assessment/m185-wrapper-contract-verification.md
- archive/data-legacy/architecture-assessment/m185-wrapper-extraction-outcomes.md
- archive/data-legacy/architecture-assessment/m185-wrapper-ratchet-decision.md
- archive/data-legacy/architecture-assessment/m185-wrapper-ratchet-verification.md
- archive/data-legacy/architecture-assessment/m186-catalog-safety-contract.md
- archive/data-legacy/architecture-assessment/m186-catalog-safety-verification.md
- archive/data-legacy/architecture-assessment/m186-catalog-verifier-boundary-pilot.md
- archive/data-legacy/architecture-assessment/m186-catalog-verifier-impact.md
- archive/data-legacy/architecture-assessment/m186-gitnexus-baseline.md
- archive/data-legacy/architecture-assessment/m186-guardrail-baseline.md
- archive/data-legacy/architecture-assessment/m186-m027-m030-catalog-drift-diagnosis.md
- archive/data-legacy/architecture-assessment/m186-m027-m030-catalog-drift-remediation.md
- archive/data-legacy/architecture-assessment/m186-m027-m030-catalog-drift-verification.md
- archive/data-legacy/architecture-assessment/m186-m055-manifest-impact.md
- archive/data-legacy/architecture-assessment/m186-m055-manifest-pilot-result.md
- archive/data-legacy/architecture-assessment/m186-m055-manifest-pilot-verification.md
- archive/data-legacy/architecture-assessment/m186-m055deep-manifest-impact.md
- archive/data-legacy/architecture-assessment/m186-m055deep-manifest-no-move.md
- archive/data-legacy/architecture-assessment/m186-m055deep-manifest-verification.md
- archive/data-legacy/architecture-assessment/m186-m058-m059-manifest-impact.md
- archive/data-legacy/architecture-assessment/m186-m058-m059-manifest-no-move.md
- archive/data-legacy/architecture-assessment/m186-m058-m059-manifest-verification.md
- archive/data-legacy/architecture-assessment/m186-manifest-atomic-writer-impact.md
- archive/data-legacy/architecture-assessment/m186-manifest-atomic-writer-verification.md
- archive/data-legacy/architecture-assessment/m186-manifest-downstream-scope.md
- archive/data-legacy/architecture-assessment/m186-manifest-lifecycle-baseline-verification.md
- archive/data-legacy/architecture-assessment/m186-manifest-lifecycle-contract.md
- archive/data-legacy/architecture-assessment/m186-manifest-lifecycle-gitnexus.md
- archive/data-legacy/architecture-assessment/m186-manifest-ratchet-transition-contract.md
- archive/data-legacy/architecture-assessment/m186-manifest-ratchet-transition-inputs.md
- archive/data-legacy/architecture-assessment/m186-manifest-ratchet-transition-verification.md
- archive/data-legacy/architecture-assessment/m186-manifest-wave-closeout-verification.md
- archive/data-legacy/architecture-assessment/m186-manifest-wave-closeout.md
- archive/data-legacy/architecture-assessment/m186-s16-downstream-readiness.md
- archive/data-legacy/architecture-assessment/m186-s16-integrated-verification.md
- archive/data-legacy/architecture-assessment/m186-s16-integration-scope.md
- archive/data-legacy/architecture-assessment/m186-s17-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m186-s17-evidence-index.md
- archive/data-legacy/architecture-assessment/m186-s17-validation-prep.md
- archive/data-legacy/architecture-assessment/m186-s18-final-gates.md
- archive/data-legacy/architecture-assessment/m186-s18-s19-validation-handoff.md
- archive/data-legacy/architecture-assessment/m186-s18-validation-rehearsal.md
- archive/data-legacy/architecture-assessment/m186-s19-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m186-s19-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m186-s19-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m186-scope-lock.md
- archive/data-legacy/architecture-assessment/m186-validation-evidence-builder-impact.md
- archive/data-legacy/architecture-assessment/m186-validation-evidence-builder-result.md
- archive/data-legacy/architecture-assessment/m186-validation-evidence-builder-verification.md
- archive/data-legacy/architecture-assessment/m186-validation-evidence-path-pilot.md
- archive/data-legacy/architecture-assessment/m186-verifier-contract-baseline.md
- archive/data-legacy/architecture-assessment/m186-verifier-impact-map.md
- archive/data-legacy/architecture-assessment/m186-verifier-wave-outcomes.md
- archive/data-legacy/architecture-assessment/m186-verifier-wave-verification.md
- archive/data-legacy/architecture-assessment/m186-verifier-wrapper-ratchet.md
- archive/data-legacy/architecture-assessment/m187-canonical-baseline-transition-delta.md
- archive/data-legacy/architecture-assessment/m187-canonical-baseline-update.md
- archive/data-legacy/architecture-assessment/m187-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m187-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m187-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m187-m055-family-impact-refresh.md
- archive/data-legacy/architecture-assessment/m187-m055-family-movement.md
- archive/data-legacy/architecture-assessment/m187-m055-family-transition-verification.md
- archive/data-legacy/architecture-assessment/m187-m058-m059-impact-refresh.md
- archive/data-legacy/architecture-assessment/m187-m058-m059-movement.md
- archive/data-legacy/architecture-assessment/m187-m058-m059-transition-verification.md
- archive/data-legacy/architecture-assessment/m187-residual-impact-map.md
- archive/data-legacy/architecture-assessment/m187-s04-integrated-guardrails.md
- archive/data-legacy/architecture-assessment/m187-transition-ratchet-decision.md
- archive/data-legacy/architecture-assessment/m187-transition-verification-plan.md
- archive/data-legacy/architecture-assessment/m188-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m188-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m188-gitnexus-scope-lock.md
- archive/data-legacy/architecture-assessment/m188-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m188-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m188-s02-current-gate-baseline.md
- archive/data-legacy/architecture-assessment/m188-s02-focused-test-baseline.md
- archive/data-legacy/architecture-assessment/m188-s02-validator-baseline.md
- archive/data-legacy/architecture-assessment/m188-s03-chunk-evidence-readiness.md
- archive/data-legacy/architecture-assessment/m188-s03-parser-chunk-readiness.md
- archive/data-legacy/architecture-assessment/m188-s03-source-boundary-readiness.md
- archive/data-legacy/architecture-assessment/m188-validator-command-map.md
- archive/data-legacy/architecture-assessment/m189-ablation-protocol.md
- archive/data-legacy/architecture-assessment/m189-command-map.md
- archive/data-legacy/architecture-assessment/m189-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m189-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m189-gitnexus-metrics-scope.md
- archive/data-legacy/architecture-assessment/m189-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m189-metric-contract.md
- archive/data-legacy/architecture-assessment/m189-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m189-s02-metric-contract-verification.md
- archive/data-legacy/architecture-assessment/m189-s02-metric-test-baseline.md
- archive/data-legacy/architecture-assessment/m189-s03-ablation-protocol-verification.md
- archive/data-legacy/architecture-assessment/m189-s03-ablation-test-baseline.md
- archive/data-legacy/architecture-assessment/m190-bounded-metrics-execution-summary.md
- archive/data-legacy/architecture-assessment/m190-bounded-selection-command-map.md
- archive/data-legacy/architecture-assessment/m190-expected-metric-outputs.md
- archive/data-legacy/architecture-assessment/m190-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m190-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m190-gitnexus-execution-scope.md
- archive/data-legacy/architecture-assessment/m190-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m190-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m190-s02-pre-execution-verification.md
- archive/data-legacy/architecture-assessment/m190-s03-local-validator-results.md
- archive/data-legacy/architecture-assessment/m190-s03-metric-ablation-results.md
- archive/data-legacy/architecture-assessment/m191-expected-parser-outputs.md
- archive/data-legacy/architecture-assessment/m191-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m191-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m191-gitnexus-parser-scope.md
- archive/data-legacy/architecture-assessment/m191-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m191-m031-catalog-backed-replay-report.md
- archive/data-legacy/architecture-assessment/m191-parser-command-map.md
- archive/data-legacy/architecture-assessment/m191-parser-readiness-expansion-summary.md
- archive/data-legacy/architecture-assessment/m191-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m191-s02-pre-execution-verification.md
- archive/data-legacy/architecture-assessment/m191-s03-parser-test-results.md
- archive/data-legacy/architecture-assessment/m191-s03-readiness-validator-results.md
- archive/data-legacy/architecture-assessment/m192-expected-graph-review-outputs.md
- archive/data-legacy/architecture-assessment/m192-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m192-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m192-gitnexus-graph-scope.md
- archive/data-legacy/architecture-assessment/m192-graph-command-map.md
- archive/data-legacy/architecture-assessment/m192-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m192-import-boundary-results.md
- archive/data-legacy/architecture-assessment/m192-review-post-check-result.md
- archive/data-legacy/architecture-assessment/m192-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m192-s02-pre-execution-verification.md
- archive/data-legacy/architecture-assessment/m193-command-decision.md
- archive/data-legacy/architecture-assessment/m193-command-verification-result.md
- archive/data-legacy/architecture-assessment/m193-expected-command-outputs.md
- archive/data-legacy/architecture-assessment/m193-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m193-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m193-gitnexus-command-scope.md
- archive/data-legacy/architecture-assessment/m193-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m193-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m193-s02-pre-execution-verification.md
- archive/data-legacy/architecture-assessment/m193-shim-retirement-test-results.md
- archive/data-legacy/architecture-assessment/m194-correction-results.md
- archive/data-legacy/architecture-assessment/m194-expected-correction-map.md
- archive/data-legacy/architecture-assessment/m194-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m194-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m194-governance-regression-results.md
- archive/data-legacy/architecture-assessment/m194-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m194-reference-scope.md
- archive/data-legacy/architecture-assessment/m194-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m194-s02-pre-edit-verification.md
- archive/data-legacy/architecture-assessment/m195-active-boundary-inventory.md
- archive/data-legacy/architecture-assessment/m195-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m195-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m195-gitnexus-boundary-context.md
- archive/data-legacy/architecture-assessment/m195-requirement-outcomes.md
- archive/data-legacy/architecture-assessment/m195-s01-scope-map.md
- archive/data-legacy/architecture-assessment/m195-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s02-contract-baseline.md
- archive/data-legacy/architecture-assessment/m195-s02-contract-verification.md
- archive/data-legacy/architecture-assessment/m195-s02-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s03-queue-baseline.md
- archive/data-legacy/architecture-assessment/m195-s03-queue-verification.md
- archive/data-legacy/architecture-assessment/m195-s03-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s04-failure-baseline.md
- archive/data-legacy/architecture-assessment/m195-s04-failure-verification.md
- archive/data-legacy/architecture-assessment/m195-s04-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s05-resume-baseline.md
- archive/data-legacy/architecture-assessment/m195-s05-resume-verification.md
- archive/data-legacy/architecture-assessment/m195-s05-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s06-gitnexus-continuity-map.md
- archive/data-legacy/architecture-assessment/m195-s06-no-write-boundary-audit.md
- archive/data-legacy/architecture-assessment/m195-s06-queue-continuity-matrix.md
- archive/data-legacy/architecture-assessment/m195-s06-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s07-no-backend-import-audit.md
- archive/data-legacy/architecture-assessment/m195-s07-projection-port-baseline.md
- archive/data-legacy/architecture-assessment/m195-s07-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s08-networkx-baseline.md
- archive/data-legacy/architecture-assessment/m195-s08-no-write-audit.md
- archive/data-legacy/architecture-assessment/m195-s08-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s09-backend-seam-baseline.md
- archive/data-legacy/architecture-assessment/m195-s09-no-write-audit.md
- archive/data-legacy/architecture-assessment/m195-s09-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s10-no-write-audit.md
- archive/data-legacy/architecture-assessment/m195-s10-projection-handoff-baseline.md
- archive/data-legacy/architecture-assessment/m195-s10-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s11-no-write-audit.md
- archive/data-legacy/architecture-assessment/m195-s11-schema-baseline.md
- archive/data-legacy/architecture-assessment/m195-s11-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s12-e2e-baseline.md
- archive/data-legacy/architecture-assessment/m195-s12-no-write-audit.md
- archive/data-legacy/architecture-assessment/m195-s12-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s13-governance-audit.md
- archive/data-legacy/architecture-assessment/m195-s13-governance-baseline.md
- archive/data-legacy/architecture-assessment/m195-s13-scope-verification.md
- archive/data-legacy/architecture-assessment/m195-s14-scope-verification.md
- archive/data-legacy/architecture-assessment/m196-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m196-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m196-requirement-outcomes.md
- archive/data-legacy/architecture-assessment/m196-s01-command-artifact-inventory.md
- archive/data-legacy/architecture-assessment/m196-s01-compatibility-plan.md
- archive/data-legacy/architecture-assessment/m196-s01-impact-gate.md
- archive/data-legacy/architecture-assessment/m196-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m196-s02-contract-audit.md
- archive/data-legacy/architecture-assessment/m196-s02-scope-verification.md
- archive/data-legacy/architecture-assessment/m196-s02-staged-validation-baseline.md
- archive/data-legacy/architecture-assessment/m196-s03-queue-resilience-baseline.md
- archive/data-legacy/architecture-assessment/m196-s03-queue-resilience-evidence.md
- archive/data-legacy/architecture-assessment/m196-s03-scope-verification.md
- archive/data-legacy/architecture-assessment/m196-s04-run-artifact-audit.md
- archive/data-legacy/architecture-assessment/m196-s04-run-artifact-baseline.md
- archive/data-legacy/architecture-assessment/m196-s04-scope-verification.md
- archive/data-legacy/architecture-assessment/m196-s05-governance-audit.md
- archive/data-legacy/architecture-assessment/m196-s05-governance-baseline.md
- archive/data-legacy/architecture-assessment/m196-s05-scope-verification.md
- archive/data-legacy/architecture-assessment/m196-s06-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m197-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m197-operator-handoff.md
- archive/data-legacy/architecture-assessment/m197-requirement-outcomes.md
- archive/data-legacy/architecture-assessment/m197-s01-impact-risk-matrix.md
- archive/data-legacy/architecture-assessment/m197-s01-reactive-seam-inventory.md
- archive/data-legacy/architecture-assessment/m197-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s01-wave-dependency-map.md
- archive/data-legacy/architecture-assessment/m197-s02-contract-audit.md
- archive/data-legacy/architecture-assessment/m197-s02-reactive-event-contract.md
- archive/data-legacy/architecture-assessment/m197-s02-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s03-baseline-audit.md
- archive/data-legacy/architecture-assessment/m197-s03-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s03-sync-baseline.md
- archive/data-legacy/architecture-assessment/m197-s04-additive-compatibility-audit.md
- archive/data-legacy/architecture-assessment/m197-s04-async-runner-boundary.md
- archive/data-legacy/architecture-assessment/m197-s04-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s05-concurrency-audit.md
- archive/data-legacy/architecture-assessment/m197-s05-runner-impact.md
- archive/data-legacy/architecture-assessment/m197-s05-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s06-failure-semantics-audit.md
- archive/data-legacy/architecture-assessment/m197-s06-failure-semantics-boundary.md
- archive/data-legacy/architecture-assessment/m197-s06-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s07-retry-lease-audit.md
- archive/data-legacy/architecture-assessment/m197-s07-retry-lease-boundary.md
- archive/data-legacy/architecture-assessment/m197-s07-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s08-lineage-safety-audit.md
- archive/data-legacy/architecture-assessment/m197-s08-lineage-safety-boundary.md
- archive/data-legacy/architecture-assessment/m197-s08-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s09-dry-run-audit.md
- archive/data-legacy/architecture-assessment/m197-s09-dry-run-boundary.md
- archive/data-legacy/architecture-assessment/m197-s09-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s10-queue-compatibility-audit.md
- archive/data-legacy/architecture-assessment/m197-s10-queue-compatibility-boundary.md
- archive/data-legacy/architecture-assessment/m197-s10-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s11-realistic-rehearsal-audit.md
- archive/data-legacy/architecture-assessment/m197-s11-realistic-rehearsal-boundary.md
- archive/data-legacy/architecture-assessment/m197-s11-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s12-governance-audit.md
- archive/data-legacy/architecture-assessment/m197-s12-governance-boundary.md
- archive/data-legacy/architecture-assessment/m197-s12-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s13-operator-handoff-audit.md
- archive/data-legacy/architecture-assessment/m197-s13-operator-handoff-boundary.md
- archive/data-legacy/architecture-assessment/m197-s13-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s14-final-compatibility-boundary.md
- archive/data-legacy/architecture-assessment/m197-s14-final-compatibility-evidence.md
- archive/data-legacy/architecture-assessment/m197-s14-final-safety-audit.md
- archive/data-legacy/architecture-assessment/m197-s14-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s15-scope-verification.md
- archive/data-legacy/architecture-assessment/m197-s15-validation-readiness-boundary.md
- archive/data-legacy/architecture-assessment/m198-final-closeout-readiness.md
- archive/data-legacy/architecture-assessment/m198-final-validation-evidence.md
- archive/data-legacy/architecture-assessment/m198-gsd-validation-result.md
- archive/data-legacy/architecture-assessment/m198-operator-readiness-runbook.md
- archive/data-legacy/architecture-assessment/m198-requirement-outcomes.md
- archive/data-legacy/architecture-assessment/m198-s01-impact-risk-matrix.md
- archive/data-legacy/architecture-assessment/m198-s01-readiness-seam-inventory.md
- archive/data-legacy/architecture-assessment/m198-s01-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s01-wave-dependency-map.md
- archive/data-legacy/architecture-assessment/m198-s02-producer-readiness-audit.md
- archive/data-legacy/architecture-assessment/m198-s02-readiness-evidence-contract.md
- archive/data-legacy/architecture-assessment/m198-s02-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s03-dry-run-probe-audit.md
- archive/data-legacy/architecture-assessment/m198-s03-dry-run-probe-boundary.md
- archive/data-legacy/architecture-assessment/m198-s03-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s04-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s04-sync-rehearsal-audit.md
- archive/data-legacy/architecture-assessment/m198-s04-sync-rehearsal-boundary.md
- archive/data-legacy/architecture-assessment/m198-s05-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s05-smoke-boundary-audit.md
- archive/data-legacy/architecture-assessment/m198-s05-smoke-boundary.md
- archive/data-legacy/architecture-assessment/m198-s06-graph-readiness-audit.md
- archive/data-legacy/architecture-assessment/m198-s06-graph-readiness-boundary.md
- archive/data-legacy/architecture-assessment/m198-s06-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s07-drift-classifier-audit.md
- archive/data-legacy/architecture-assessment/m198-s07-drift-classifier-boundary.md
- archive/data-legacy/architecture-assessment/m198-s07-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s08-evidence-index-audit.md
- archive/data-legacy/architecture-assessment/m198-s08-evidence-index-boundary.md
- archive/data-legacy/architecture-assessment/m198-s08-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s09-operator-diagnostics-audit.md
- archive/data-legacy/architecture-assessment/m198-s09-operator-diagnostics-boundary.md
- archive/data-legacy/architecture-assessment/m198-s09-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s10-readiness-report-audit.md
- archive/data-legacy/architecture-assessment/m198-s10-readiness-report-boundary.md
- archive/data-legacy/architecture-assessment/m198-s10-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s11-no-write-governance-audit.md
- archive/data-legacy/architecture-assessment/m198-s11-no-write-governance-boundary.md
- archive/data-legacy/architecture-assessment/m198-s11-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s12-gitnexus-impact-gates-audit.md
- archive/data-legacy/architecture-assessment/m198-s12-gitnexus-impact-gates-boundary.md
- archive/data-legacy/architecture-assessment/m198-s12-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s13-realistic-rehearsal-audit.md
- archive/data-legacy/architecture-assessment/m198-s13-realistic-rehearsal-boundary.md
- archive/data-legacy/architecture-assessment/m198-s13-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s14-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s14-smoke-parity-audit.md
- archive/data-legacy/architecture-assessment/m198-s14-smoke-parity-boundary.md
- archive/data-legacy/architecture-assessment/m198-s15-disabled-backend-safety-audit.md
- archive/data-legacy/architecture-assessment/m198-s15-disabled-backend-safety-boundary.md
- archive/data-legacy/architecture-assessment/m198-s15-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s16-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s16-validation-package-audit.md
- archive/data-legacy/architecture-assessment/m198-s16-validation-package-boundary.md
- archive/data-legacy/architecture-assessment/m198-s17-operator-runbook-audit.md
- archive/data-legacy/architecture-assessment/m198-s17-operator-runbook-boundary.md
- archive/data-legacy/architecture-assessment/m198-s17-scope-verification.md
- archive/data-legacy/architecture-assessment/m198-s18-final-closeout-boundary.md
- archive/data-legacy/architecture-assessment/write-path-inventory-canonical-delta.md
- archive/data-legacy/architecture-assessment/write-path-inventory-canonical.md
- archive/data-legacy/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline-recovery-report.md
- archive/data-legacy/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md
- archive/data-legacy/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/catalog-report.md
- archive/data-legacy/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-report.md
- archive/data-legacy/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-report.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/catalog-report.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/conversion-quality-report.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/current-pipeline-baseline-report.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay-report.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay-verification-report.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-report.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/provenance-riskratchet-gate/maintainability-diagnostic.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/provenance-riskratchet-gate/provenance-riskratchet-gate-report.md
- archive/data-legacy/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-report.md
- archive/data-legacy/article_corpora/m028-universal-loader-runtime-smoke-v1/acquisition-report.md
- archive/data-legacy/article_corpora/m028-universal-loader-runtime-smoke-v1/hermes-digest-projection-report.md
- archive/data-legacy/article_corpora/m028-universal-loader-runtime-smoke-v1/pdf-acquisition-report.md
- archive/data-legacy/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/hermes-digest-projection-report.md
- archive/data-legacy/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/pdf-acquisition-report.md
- archive/data-legacy/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts/universal-loader-evidence-report.md
- archive/data-legacy/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/smoke-replay-closeout-report.md
- archive/data-legacy/article_corpora/m028-universal-loader-runtime-smoke-v1/universal-loader-evidence-report.md
- archive/data-legacy/article_corpora/m029-pipeline-architecture-audit-v1/intake-report.md
- archive/data-legacy/article_corpora/m029-unified-corpus-v1/conversion-quality-report.md
- archive/data-legacy/article_corpora/m029-unified-corpus-v1/post-validation-remediation/post-validation-remediation-report.md
- archive/data-legacy/article_corpora/m029-unified-corpus-v1/readiness-report.md
- archive/data-legacy/article_corpora/m029-unified-corpus-v1/replay-report.md
- archive/data-legacy/article_corpora/m029-unified-corpus-v1/runtime-smoke-report.md
- archive/data-legacy/article_corpora/m029-unified-corpus-v1/selection-report.md
- archive/data-legacy/article_corpora/m029-unified-corpus-v1/source-acquisition-report.md
- archive/data-legacy/article_corpora/m029-unified-corpus-v1/validation-remediation/remediation-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence-closeout-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence/chunk-evidence-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/conversion-quality/conversion-quality-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/graph-readiness-review/arxiv_cs-cl_2507.19457_arxiv_pdf-review.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/graph-readiness-review/independent-review-summary.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/import-boundary-rehearsal/import-boundary-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/loader-evidence-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/m031-continuity-audit.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/parser-conversion-closeout-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/progression-matrix.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/replay-closeout-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/s05-closeout-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/source-acquisition-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/validation-remediation/s02-replay-closeout-report.md
- archive/data-legacy/article_corpora/m031-catalog-backed-replay-v1/validation-remediation/validation-remediation-report.md
- archive/data-legacy/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-report.md
- archive/data-legacy/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.md
- archive/data-legacy/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.md
- archive/data-legacy/article_corpora/m033-combined-parser-architecture-v1/synthesis-evidence-matrix.md
- archive/data-legacy/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.md
- archive/data-legacy/article_corpora/m033-current-parser-baseline-v1/current-baseline-closeout.md
- archive/data-legacy/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.md
- archive/data-legacy/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.md
- archive/data-legacy/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.md
- archive/data-legacy/article_corpora/m033-external-parser-quality-plan-v1/adoption-and-rollback-criteria.md
- archive/data-legacy/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.md
- archive/data-legacy/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md
- archive/data-legacy/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md
- archive/data-legacy/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-report.md
- archive/data-legacy/article_corpora/m033-grobid-probe-v1/grobid-closeout-report.md
- archive/data-legacy/article_corpora/m033-grobid-probe-v1/grobid-contract-mapping.md
- archive/data-legacy/article_corpora/m033-grobid-probe-v1/grobid-runtime-runbook.md
- archive/data-legacy/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-report.md
- archive/data-legacy/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-report.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/ad-hoc-smoke/hybrid-backend-page1/original.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/ad-hoc-smoke/py313-wrapper-page1/original.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/ad-hoc-smoke/reca-first2/original.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-contract-mapping.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-report.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-runbook.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper/2507.19457/hybrid/original.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper/2512.24601/hybrid/original.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper/2605.26525v1/hybrid/original.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-output/hybrid/original.md
- archive/data-legacy/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-output/java-only/original.md
- archive/data-legacy/article_corpora/m033-quantmind-pattern-study-v1/quantmind-closeout-report.md
- archive/data-legacy/article_corpora/m033-quantmind-pattern-study-v1/quantmind-daily-archive-pattern-map.md
- archive/data-legacy/article_corpora/m033-quantmind-pattern-study-v1/quantmind-implemented-vs-vision.md
- archive/data-legacy/article_corpora/m033-quantmind-pattern-study-v1/quantmind-runtime-decision.md
- archive/data-legacy/pipeline-script-architecture/README.md
- archive/data-legacy/pipeline-script-architecture/acceptance-ingest-report.md
- archive/data-legacy/r024-10-document-corpus-v1/R024-COVERAGE.md
- archive/data-legacy/r024-10-document-corpus-v1/quality-comparison-5-vs-10.md
- archive/data-legacy/r024-20-document-corpus-v1/R024-COVERAGE.md
- archive/data-legacy/r024-20-document-corpus-v1/quality-comparison-10-vs-20.md
- archive/data-legacy/r024-218-document-corpus-v1/R024-COVERAGE.md
- archive/data-legacy/r024-53-document-corpus-v1/R024-COVERAGE.md
- archive/data-legacy/r024-53-document-corpus-v1/quality-comparison-20-vs-53.md
- archive/data-legacy/r024-catalog-ingest-migration/R024-MIGRATION.md
- archive/data-legacy/r024-catalog-ingest-migration/audit.md
- archive/data-legacy/r024-entity-scale-corpus-v1/R024-COVERAGE.md
- archive/data-legacy/r024-entity-scale-corpus-v1/comparison-5-entities-vs-10-entities.md
- archive/data-legacy/r024-entity-scale-corpus-v1/entity-schema.md
- archive/data-legacy/test-architecture-alignment/README.md
- archive/data-legacy/test-architecture-alignment/m131-m061-s02-fixture-diagnosis.md
- archive/data-legacy/test-architecture-alignment/m133-candidate-selection.md
- archive/data-legacy/test-architecture-alignment/m133-debt-recon.md
- archive/data-legacy/test-architecture-alignment/m133-pyrefly-suppression-removals.md
- archive/data-legacy/test-architecture-alignment/m133-pyrefly-suppressions.md
- archive/data-legacy/test-architecture-alignment/m133-unknown-reduction.md
- archive/data-legacy/test-architecture-alignment/m134-m052-diagnosis.md
- archive/data-legacy/test-architecture-alignment/m134-m052-repair.md
- archive/data-legacy/test-architecture-alignment/m135-m052-promotion-candidate.md
- archive/data-legacy/test-architecture-alignment/m135-m052-promotion.md
- archive/data-legacy/test-architecture-alignment/m136-dynamic-ratchet-candidate.md
- archive/data-legacy/test-architecture-alignment/m136-dynamic-ratchet.md
- archive/data-legacy/test-architecture-alignment/m137-dynamic-ratchet-candidate.md
- archive/data-legacy/test-architecture-alignment/m137-dynamic-ratchet.md
- archive/data-legacy/test-architecture-alignment/m138-dynamic-ratchet-candidate.md
- archive/data-legacy/test-architecture-alignment/m138-dynamic-ratchet.md
- archive/data-legacy/test-architecture-alignment/m139-dynamic-ratchet-candidate.md
- archive/data-legacy/test-architecture-alignment/m139-dynamic-ratchet.md
- archive/data-legacy/test-architecture-alignment/m140-dynamic-ratchet-candidate.md
- archive/data-legacy/test-architecture-alignment/m140-dynamic-ratchet.md
- archive/data-legacy/test-architecture-alignment/m141-dspy-ratchet.md
- archive/data-legacy/test-architecture-alignment/m141-dspy-repair.md
- archive/data-legacy/test-architecture-alignment/m142-boundary-replay-ratchet.md
- archive/data-legacy/test-architecture-alignment/m143-evidence-replay-ratchet.md
- archive/data-legacy/test-architecture-alignment/m144-requirement-scope-ratchet.md
- archive/data-legacy/test-architecture-alignment/m145-m026-requirement-scope-ratchet.md
- archive/data-legacy/test-architecture-alignment/m146-m027-current-pipeline-baseline-ratchet.md
- archive/data-legacy/test-architecture-alignment/m147-m027-batch-ratchet.md
- archive/data-legacy/test-architecture-alignment/m148-m028-batch-ratchet.md
- archive/data-legacy/test-architecture-alignment/m149-validation-remediation-batch-ratchet.md
- archive/data-legacy/test-architecture-alignment/m150-m056-wave-analyzer-batch-ratchet.md
- archive/data-legacy/test-architecture-alignment/m151-m060-batch-ratchet.md
- archive/data-legacy/test-architecture-alignment/m152-connectivity-sidecar-batch-ratchet.md
- archive/data-legacy/test-architecture-alignment/m153-pipeline-audit-batch-ratchet.md
- archive/data-legacy/test-architecture-alignment/m154-expanded-batch-ratchet.md
- archive/data-legacy/test-architecture-alignment/m161-dynamic-repair.md
- archive/data-legacy/test-architecture-alignment/m161-dynamic-triage.md
- archive/data-legacy/test-architecture-alignment/m162-dynamic-repairs.md
- archive/data-legacy/test-architecture-alignment/m162-sync-policy-docs.md
- archive/data-legacy/test-architecture-alignment/m162-timeout-probes.md
- archive/data-legacy/test-architecture-alignment/pilot-classification.md
- archive/data-legacy/test-architecture-alignment/ratchet-candidates-m130.md
- archive/data-legacy/test-architecture-alignment/ratchet-candidates-m132.md
- archive/data-legacy/test-architecture-alignment/ratchet-candidates.md
- archive/data-legacy/test-architecture-alignment/test-architecture-guardrail.md
- archive/data-legacy/test-architecture-alignment/test-architecture-inventory.md
- archive/m451-cleanup/data-corpora-m033grobidprobe-dup-20260827/m033-grobid-probe-v1/grobid-closeout-report.md
- archive/m451-cleanup/data-corpora-m033grobidprobe-dup-20260827/m033-grobid-probe-v1/grobid-contract-mapping.md
- archive/m451-cleanup/data-corpora-m033grobidprobe-dup-20260827/m033-grobid-probe-v1/grobid-runtime-runbook.md
- archive/package-layout-shims/wave-01/manifest.md
- archive/package-rename-waves/wave-00/manifest.md
- archive/package-rename-waves/wave-01/manifest.md
- archive/package-rename-waves/wave-02/manifest.md
- archive/package-rename-waves/wave-03/manifest.md
- archive/package-rename-waves/wave-04/manifest.md
- archive/package-rename-waves/wave-05/manifest.md
- archive/package-rename-waves/wave-06/manifest.md
- archive/package-rename-waves/wave-07/manifest.md
- archive/package-rename-waves/wave-08/manifest.md
- archive/package-rename-waves/wave-09/manifest.md
- archive/package-rename-waves/wave-10/manifest.md
- archive/package-rename-waves/wave-11/manifest.md
- archive/package-rename-waves/wave-12/manifest.md
- archive/package-rename-waves/wave-13/manifest.md
- archive/package-rename-waves/wave-14/manifest.md
- archive/package-rename-waves/wave-15/manifest.md
- archive/package-rename-waves/wave-16/manifest.md
- archive/package-rename-waves/wave-17/manifest.md
- archive/package-rename-waves/wave-18/manifest.md
- archive/package-rename-waves/wave-19/manifest.md
- archive/package-rename-waves/wave-20/manifest.md
- archive/python-era-cold-s04/manifest.md
- artifacts/OKF.md
- artifacts/learning/policy-corpus-m517/README.md
- artifacts/learning/policy-corpus-m517/protocol-corrections.md
- artifacts/learning/policy-corpus-m517/reviews/evaluation.md
- artifacts/learning/policy-corpus-m517/reviews/gsd-closeout-path.md
- artifacts/learning/policy-corpus-m517/reviews/m517-cycle-result-review.md
- artifacts/learning/policy-corpus-m517/reviews/m517-final-q6-closeout-audit.md
- artifacts/learning/policy-corpus-m517/reviews/m517-final-readiness-review.md
- artifacts/learning/policy-corpus-m517/reviews/m517-gitnexus-recovery.md
- artifacts/learning/policy-corpus-m517/reviews/m517-independent-review.md
- artifacts/learning/policy-corpus-m517/reviews/m517-narrow-final-source-review.md
- artifacts/learning/policy-corpus-m517/reviews/m517-source-review-interpretation.md
- artifacts/learning/policy-corpus-m517/reviews/runtime.md
- artifacts/parse_compat/historical/README.md
- artifacts/parse_compat/native_body_additive_assignment.evidence.md
- artifacts/parse_compat/native_body_boundary_grounding.evidence.md
- artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
- artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
- artifacts/parse_compat/native_body_identity_intersection.addendum.md
- artifacts/parse_compat/native_body_order_lineage.addendum.md
- artifacts/parse_compat/native_body_order_lineage.evidence.md
- artifacts/parse_compat/native_body_third_route.addendum.md
- artifacts/parse_compat/native_body_uncertainty_split.addendum.md
- artifacts/parse_compat/native_body_uncertainty_split.evidence.md
- artifacts/parse_compat/s05_t01_p1_rung_checked_run/EVIDENCE.md
- artifacts/parse_compat/s05_t01_p1_rung_checked_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/s05_t01_p1_rung_checked_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
- artifacts/parse_compat/s05_t01_p1_rung_checked_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
- artifacts/parse_compat/s05_t01_repair2_cleanup_run/prior_runbook_repair2.NOTE.md
- artifacts/parse_compat/s05_t01_report_contract_run/EVIDENCE.md
- artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/artifacts/parse_compat/native_body_order_lineage.addendum.md
- artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/artifacts/parse_compat/native_body_order_lineage.evidence.md
- artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/artifacts/parse_compat/native_body_uncertainty_split.evidence.md
- artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/artifacts/parse_compat/native_body_additive_assignment.evidence.md
- artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/artifacts/parse_compat/native_body_boundary_grounding.evidence.md
- artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
- artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
- artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/artifacts/parse_compat/native_body_identity_intersection.addendum.md
- artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/artifacts/parse_compat/native_body_order_lineage.evidence.md
- artifacts/parse_compat/s05_t01_route_c_preservation.20261001T020513Z/artifacts/parse_compat/native_body_uncertainty_split.evidence.md
- artifacts/parse_compat/s05_t01_third_route_addendum_preservation.20261001T025608Z/artifacts/parse_compat/native_body_third_route.addendum.md
- artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
- artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
- artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/native_body_order_lineage.evidence.md
- artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/s05_t01_p1_rung_checked_run/EVIDENCE.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_additive_assignment.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_boundary_grounding.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_order_lineage.addendum.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_order_lineage.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_uncertainty_split.addendum.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/native_body_uncertainty_split.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_p1_rung_checked_run/EVIDENCE.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_p1_rung_checked_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_p1_rung_checked_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_p1_rung_checked_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_repair2_cleanup_run/prior_runbook_repair2.NOTE.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_report_contract_run/EVIDENCE.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/artifacts/parse_compat/native_body_order_lineage.addendum.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/artifacts/parse_compat/native_body_order_lineage.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_report_contract_run/preserved_before/artifacts/parse_compat/native_body_uncertainty_split.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.prior_repair.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/native_body_character_scope_bound.repair2_run.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/native_body_order_lineage.evidence.md
- artifacts/parse_compat/s05_t01_v5_preservation.20261001T011654Z/artifacts/parse_compat/s05_t01_uncertainty_split_run/preserved_before/artifacts/parse_compat/s05_t01_p1_rung_checked_run/EVIDENCE.md
- artifacts/parse_compat/s05_t09_final_verify/README.md
- crates/OKF.md
- data/OKF.md
- data/README.md
- data/gold_standard/OKF.md
- data/gold_standard/README.md
- data/graph_tasks/OKF.md
- data/graph_tasks/README.md
- data/graphs/OKF.md
- data/graphs/README.md
- data/ontology/OKF.md
- data/ontology/README.md
- data/processed/README.md
- data/prompts/article-summary/baseline.v1.md
- data/prompts/article-summary/per-section.v1.md
- data/prompts/critic/system.v1.md
- data/prompts/summarizer/system.v1.md
- data/prompts/summarizer/system.v2.md
- data/schemas/OKF.md
- data/schemas/README.md
- data/sources/OKF.md
- data/sources/README.md
- data/sources/SOURCE_SCHEMA.md
- data/sources/blog/vasiliev2026knowledge-graph-memory-graph/NOTES.md
- data/sources/blog/vasiliev2026knowledge-graph-memory-graph/source/article.md
- data/sources/course/OKF.md
- doc/ADHD-ONTOLOGY-RESEARCH.md
- doc/CONFIG-EXTERNALIZATION-PLAN.md
- doc/DOCLING-FIGURE-UNDERSTANDING-RESEARCH.md
- doc/DOMAIN-REFERENCE-ARXIV.md
- doc/GRAPH-CORE-TEMPORAL-DESIGN.md
- doc/GRAPH-HEALING-SCENARIOS.md
- doc/META-OPTIMIZATION-PLAN.md
- doc/ONTOLOGY-ALIGNMENT.md
- doc/ONTOLOGY-COMPLETENESS-AUDIT.md
- doc/ONTOLOGY-DESIGN-V2.md
- doc/ONTOLOGY-DESIGN.md
- doc/PERSISTENCE-ANALYSIS.md
- doc/PROCESS-SCHEMA-P0.md
- doc/SCHEMA-LIFECYCLE-DESIGN.md
- doc/SCHEMA-VALIDATOR-DESIGN.md
- doc/STUB-AUDIT.md
- doc/adr/ADR-037-rust-architecture-ruvector-nebula.md
- doc/adr/ADR-038-agents-k1-schema-operators-core-then-modes.md
- doc/adr/ADR-039-grounded-architecture-lifecycle-validation-sources.md
- doc/adr/ADR-040-technology-stack-lock-samyama-ruvector-rvf.md
- doc/adr/ADR-041-samyama-embedded-cypher-hybrid-agentruntime-sona.md
- doc/adr/ADR-042-hycerag-hypergraph-evidence-chain.md
- doc/adr/ADR-043-research-process-plane-execution-grounded.md
- doc/adr/ADR-044-schema-lifecycle-versioning-migration-healing.md
- doc/adr/ADR-045-schema-validator-logic-relations-invariants.md
- doc/adr/ADR-046-bitemporal-fact-model.md
- doc/adr/ADR-047-conflict-detection-resolution.md
- doc/adr/ADR-048-decision-intelligence.md
- doc/adr/ADR-049-pipeline-dsl-execution-engine.md
- doc/adr/ADR-050-universal-graph-subsystem-kg-crate-family.md
- doc/adr/ADR-051-testing-strategy-overlapping-verification-contours.md
- doc/adr/ADR-052-structured-logging-tracing-dual-layer.md
- doc/adr/ADR-053-article-preparation-multi-layer-document-model.md
- doc/adr/ADR-054-mcp-server-knowledge-base-access.md
- doc/adr/ADR-055-article-context-assurance-plane.md
- doc/adr/ADR-056-universal-article-acquisition-load-plane.md
- doc/adr/ADR-057-non-article-knowledge-unit-ontology.md
- doc/adr/ADR-058-article-revision-expression-ontology.md
- doc/adr/ADR-059-prompt-execution-optimizer-fsm.md
- doc/adr/ADR-060-scientific-figure-understanding-pipeline.md
- doc/adr/ADR-061-readonly-identity-graph-plane-and-agent-fsm.md
- doc/adr/ADR-062-figure-quote-policy-window.md
- doc/adr/ADR-063-graphwrite-deferred-by-design.md
- doc/adr/ADR-064-kg-pipeline-honest-execution-engine.md
- doc/adr/ADR-065-tei-declared-revision-binding.md
- doc/adr/ADR-066-pipeline-orchestrator-stays-product-side.md
- doc/adr/ADR-067-normalized-data-layout.md
- doc/adr/ADR-068-schema-manifest-hierarchy.md
- doc/adr/ADR-069-isc-pattern-application.md
- doc/adr/ADR-070-domain-knowledge-profiles.md
- doc/adr/ADR-071-survey-spine-and-hypergraph-knowledge-ranking.md
- doc/adr/ADR-072-xy-cut-reading-order.md
- doc/adr/ADR-073-upstream-resilience-and-hybrid-search.md
- doc/adr/ADR-074-wasm-microlora-policy-modules.md
- doc/adr/ADR-075-scholarly-parse-plane-staged-platform.md
- doc/adr/ADR-076-grobid-cascade-three-automata-graph-freeze.md
- doc/adr/ADR-077-independent-pdf-layout-input-plane.md
- doc/adr/ADR-078-candidate-learned-decision-table-policy.md
- doc/adr/ADR-079-queen-faculties-for-the-parse-policy-plane.md
- doc/adr/ADR-080-two-arm-paired-observations-for-policy-learning.md
- doc/parse-job-ledger.md
- doc/parse/bibliography-abi.md
- doc/parse/feature-provider-parity.md
- doc/parse/grobid-model-rights.md
- doc/parse/native-fulltext.md
- doc/parse/native-header.md
- doc/parse/native-performance-recovery.md
- doc/parse/native-visual-families.md
- doc/parse/policy-variant-ablation.md
- doc/parse/scalar-crf.md
- doc/research/document-model-pageindex-synthesis-2026-08-23.md
- doc/research/m463-grobid-parse-plane.md
- doc/research/m463-grobid-rust-ruvector-platform.md
- doc/research/m463-grobid-tei-lift-matrix.md
- doc/research/vasiliev-2026-three-graph-roles.md
- doc/review/HUMAN-REVIEW-TEMPLATE.md
- doc/review/M511-S08-release-decision-package.md
- doc/review/M513-S07-release-decision-package.md
- doc/review/M514-S04-T04-word-projection-repair.md
- doc/review/M516-S12-release-readiness.md
- doc/review/README.md
- doc/review/grobid-absorption-audit-2026-09-21.md
- doc/review/grobid-absorption-progress-baseline.md
- doc/review/grobid-absorption-review-2026-09-10-summary.md
- doc/review/grobid-absorption-s02-baseline-report.md
- doc/review/grobid-absorption-s02-corpus-inventory.md
- doc/review/grobid-absorption-s03-spike.md
- doc/review/grobid-parity-report.md
- doc/review/grobid-release-attribution.md
- doc/review/grobid-rollback-readiness.md
- doc/review/grobid-rust-native-integration-m519-2026-10-15.md
- doc/review/grobid-rust-recommendations-review-2026-10-11.md
- doc/review/native-pdf-m520-resource-audit.md
- doc/review/next-milestone-spec-draft.md
- doc/review/pipeline-roadmap-2026-08-22.md
- doc/review/pipeline-stage-audit-2026-08-22.md
- doc/review/policy-corpus-cycles.md
- doc/review/vendor-upgrade-m518-evidence/README.md
- doc/review/vendor-upgrade-m518.md

## User rulings

<!-- None — first audit, no ruling walk. -->

| Queue # | Ruling | User's words | Planned |
|---------|--------|--------------|---------|
| 1 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 2 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 3 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 4 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 5 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 6 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 7 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 8 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 9 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 10 | fix-doc | "Пакет: … #10 fix-doc (выверить формулировку)" (Q2 bundle, 2026-11-22) | no |
| 11 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 12 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 13 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 14 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22; строка .md, не создание logs/) | no |
| 15 | accept-drift | "Пакет: … #15/#23/#24 accept-drift с резолюцией «инициализация/мордожка 2026-11-22» verbatim" (Q2 bundle, 2026-11-22) | n/a (accept-drift) |
| 16 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 17 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 18 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 19 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 20 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 21 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 22 | fix-doc | "Попутно разрешить механические fix-doc правки задокументированной старины" (Q8 allow-fixdoc, 2026-11-22) | no |
| 23 | accept-drift | "Пакет: … #15/#23/#24 accept-drift с резолюцией «инициализация/мордожка 2026-11-22» verbatim" (Q2 bundle, 2026-11-22) | n/a (accept-drift) |
| 24 | accept-drift | "Пакет: … #15/#23/#24 accept-drift с резолюцией «инициализация/мордожка 2026-11-22» verbatim" (Q2 bundle, 2026-11-22) | n/a (accept-drift) |

## Remediation queue

| # | Doc | Claim | Verdict | Class | Suggested action |
|---|-----|-------|---------|-------|------------------|
| 1 | README.md | "Binding ADRs: doc/adr/ADR-INDEX.md (037–042)" | stale | fix-doc | Update range to 037–080 (or "see ADR-INDEX") |
| 2 | README.md | "pipeline_stage_contracts.yaml currently defines 14 stages" | stale | fix-doc | Say 19 stages, or render the count from the YAML |
| 3 | README.md | "default summary generator paper-summary-generate-default → agnes-ai/agnes-2.0-flash via 9router" | stale | fix-doc | Refresh to the current models.yaml summary bindings (summary-roles-v0.2 tier) |
| 4 | README.md | "artifacts/runtime/prompt_optimizer_readiness_m387.json binds both prompt tasks" | stale | fix-doc | Drop or repoint to the existing artifacts/runtime evidence |
| 5 | README.md | "Awesome Agentic Reasoning wave pinned at artifacts/source_recovery/awesome_agentic_reasoning_manifest.json (+assurance/.ids/.replay paths)" | stale | fix-doc | Remove the corpus-wave section or repoint to archived evidence |
| 6 | README.md | "Current extraction metrics P=0.766 R=1.000 micro-F1=0.868 (see artifacts/gold_corpus/eval_aggregate_all_sources.json)" | stale | fix-doc | Cite the surviving archive/artifacts-legacy copy |
| 7 | README.md | "Graph schema done — 31 node types, 26 indexes; `da schema-init` initializes all 26 graph indexes" | stale | fix-doc | Update to 56 (46 identity + 10 property) or render from YAML |
| 8 | README.md | "Layout: legacy/ frozen Python stub; source assets under data/article_catalog/article_catalog/arxiv/…" | stale | fix-doc | Remove stale layout entries |
| 9 | AGENTS.md | "Reverse-ADR audit enforces 7 rules: 4 artifacts-gate + 3 Rust-specific" | stale | fix-doc | Update to 65 rules in checks/adr_rules.yaml |
| 10 | AGENTS.md | ".project/ is committed; gsd-path is the active engine; user work in stash@{0}" | stale | NEEDS-USER | Claim postdates audited baseline 0f06b764; confirm at current main before editing |
| 11 | PRODUCT.md | "14-stage pipeline ledger (data/ontology/pipeline_stage_contracts.yaml)" | stale | fix-doc | Update to 19 |
| 12 | PRODUCT.md | "schema registry and validators (26 indexes, edge contracts, node schemas)" | stale | fix-doc | Update to 56 |
| 13 | PRODUCT.md | "ontology export (31 schemas → YAML)" | stale | fix-doc | Update to 43 |
| 14 | OKF.md | "Tree layout includes logs/ (structured logs, ADR-052)" | stale | fix-code | Create logs/ per ADR-052 or drop the line |
| 15 | WORKFLOW.md | "The gsd-path router reads .project/STATE.md and invokes the next skill" | stale | NEEDS-USER | .project/ absent at audited HEAD; verify against current main |
| 16 | crates/kg-ontology/README.md | "Phase A (current): skeleton crate with Severity, Violation, PropertySnapshot, format_violations" | stale | fix-doc | Rewrite Status section to the current module surface |
| 17 | crates/kg-pipeline/README.md | "Phase E: 7 tests" | stale | fix-doc | Refresh test counts |
| 18 | crates/kg-algorithms/README.md | "Phase E (current): temporal edge resolution (4-rule ADR-047); 9 tests" | stale | fix-doc | Rewrite Status to include csr/ppr/view algorithms and current counts |
| 19 | doc/adr/ADR-INDEX.md | "Python-era ADRs (ADR-001..036) … see legacy/doc/adr/" | stale | fix-doc | Drop legacy/ reference or restore stub |
| 20 | doc/OKF.md | "doc/adr/ ← Architecture Decision Records (ADR-001…067)" | stale | fix-doc | Update to ADR-037…080 |
| 21 | scripts/OKF.md | "scripts/check_project_trajectory.py legacy entry point (→ governor)" | stale | fix-doc | Remove the retired entry-point line |
| 22 | .docker/README.md | "Python research_graph.* paths frozen under legacy/, not on runtime path" | stale | fix-doc | Drop the legacy/ sentence |
| 23 | .codebase-memory/adr.md | "Generated by scripts/sync_codebase_memory_governance.py" | stale | NEEDS-USER | Generator script absent; restore it or regenerate the mirror by another means — ownership unclear |
| 24 | tools/gsd-engine-patches/README.md | "Patches applied to the installed @opengsd/gsd-pi 1.21.0 outside this repo, keeping *.pre-projection-lock-patch backups" | unverifiable | NEEDS-USER | Evidence lives outside the repo (installed npm package); inspect machine state to verify |
