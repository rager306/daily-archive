# Gap Review — 2: cross-wave integration — every figure wave 2 (T004, landing 84dd6a1fa via workspace 3d813db9e) introduced into README.md/AGENTS.md/PRODUCT.md/OKF.md/doc/OKF.md must match the real ontology/governor sources and the wave-1 reports; any contradiction between a fixed doc, its source, and a wave-1 report claim is a finding.

<!-- Cycle-2 re-review of the FIXED tree. Cycle 1 (this same file, reviewed HEAD
     7e891591a5a0da7131f61f4050bb8fd0c6dd09a3) was BLOCKED: AGENTS.md:348 still
     claimed "31 node schemas" against source 43. Owner approved patch P001 →
     T005 (landing cb1e45d3c, present in this tree, .project/review/
     PATCH-FINDINGS.md:8-13, .project/tasks/T005-node-schema-count-fix.md),
     which changed AGENTS.md:348 to "43". This cycle re-verifies the fixed
     state at HEAD 3b6ec09240150cc2ac9866f26a0bd43ca98237ab. Read-only
     git/grep/yaml-parse; no builds. -->

Reviewed HEAD: 3b6ec09240150cc2ac9866f26a0bd43ca98237ab
Gap verdict: pass
Risk: cross-wave integration — every figure wave 2 (T004, landing 84dd6a1fa via workspace 3d813db9e) introduced into README.md/AGENTS.md/PRODUCT.md/OKF.md/doc/OKF.md must match the real ontology/governor sources and the wave-1 reports; any contradiction between a fixed doc, its source, and a wave-1 report claim is a finding.
Waves checked: wave-1 (cycle-1 reports) + cycle-1 gap + patch P001/T005

## Checked evidence

- **Check**: `sed -n 348p AGENTS.md` + yaml-parse of `data/ontology/node_schemas.yaml` (list length); yaml-parse of `data/ontology/identity_indexes.yaml`, `data/ontology/property_indexes.yaml`, `data/ontology/pipeline_stage_contracts.yaml`, `scripts/governor/checks/adr_rules.yaml`; `git grep -nE '31 (node )?(schemas|types)' -- '*.md'` over tracked .md filtered by the allow-set (AGENTS.pre-path.md backup, ADR-048:179 / ADR-050:439 decision records, dated ONTOLOGY-DESIGN-V2.md:107 / ONTOLOGY-COMPLETENESS-AUDIT.md:208 snapshots, .project findings/plan/task/evidence files quoting the pattern verbatim); figure cross-read of .project/review/wave-1.cycle1.md, OKF.md, doc/OKF.md, PRODUCT.md:12; `git rev-parse HEAD`
- **Observed**: AGENTS.md:348 reads `# Export all 43 node schemas to YAML` (the exact string T005's acceptance demanded); yaml-parse: node_schemas = 43 entries (`^- label:` list = 43), identity_indexes.yaml = 46 + property_indexes.yaml = 10 → 56 indexes, pipeline_stage_contracts.yaml stages = 19, adr_rules.yaml rules = 65 — README.md:25 `43 node schemas, 56 indexes`, README.md:72 `19 stages`, AGENTS.md:285 `65 rules`, PRODUCT.md:15 `56 indexes … 43 schemas` all match the sources; zero LIVE 31-claims outside the allow-set (remaining hits only archival/quoted-evidence: AGENTS.pre-path.md:347, ADR-048:179, ADR-050:439, ONTOLOGY-DESIGN-V2.md:107, ONTOLOGY-COMPLETENESS-AUDIT.md:208, archive/ rows, .project evidence quotes); wave-1 reports assert no conflicting counts (OKF.md and doc/OKF.md carry no figures; PRODUCT.md:12 F1 figures match the prior-cycle audited values); HEAD = 3b6ec09240150cc2ac9866f26a0bd43ca98237ab with T005 landing cb1e45d3c in lineage.
- **Reference**: AGENTS.md:348,285; data/ontology/node_schemas.yaml:12; data/ontology/identity_indexes.yaml; data/ontology/property_indexes.yaml; data/ontology/pipeline_stage_contracts.yaml; scripts/governor/checks/adr_rules.yaml; README.md:25,72,469,532; PRODUCT.md:12,15; .project/review/wave-1.cycle1.md:1-91; .project/tasks/T005-node-schema-count-fix.md:70-78; wave-3.cycle1.md:8-10

## Finding

- **Found**: no contradiction remains at HEAD. (1) AGENTS.md:348 states "Export all 43 node schemas to YAML", consistent with the yaml-parse count (43) of data/ontology/node_schemas.yaml:12 and with README.md:25 / PRODUCT.md:15. (2) The codebase-wide stale-pattern sweep over tracked .md, after the allow-set exclusions, leaves no live 31-schema claim; every residual hit is a dated snapshot, a decision record, the pre-path backup, an archived artifact, or a .project file quoting the pattern as evidence/specification. (3) All other wave-2-introduced figures verified against sources (56 = 46+10 indexes, 19 stages, 65 ADR rules) and are consistent with the wave-1 reports, which assert no conflicting counts; OKF.md/doc/OKF.md carry no figures. Cycle-1's blocked finding is resolved by the owner-approved patch P001 → T005.
- **Fix direction**: none
