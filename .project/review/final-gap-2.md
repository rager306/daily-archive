# Gap Review — 2: cross-wave integration of wave-2 fix-doc counts

<!-- Written by one final gap reviewer. This file covers exactly one
     cross-wave integration risk named in the review brief. Repeat FINAL.md's
     `Member reviewed HEAD: <member> <full SHA>` lines after Reviewed HEAD. -->

Reviewed HEAD: 7e891591a5a0da7131f61f4050bb8fd0c6dd09a3
Gap verdict: blocked
Risk: cross-wave integration — every figure wave 2 (T004, landing 84dd6a1fa via workspace 3d813db9e) introduced into README.md/AGENTS.md/PRODUCT.md/OKF.md/doc/OKF.md must match the real ontology/governor sources and the wave-1 reports; any contradiction between a fixed doc, its source, and a wave-1 report claim is a finding.
Waves checked: 1, 2

## Checked evidence

- **Check**: `python3` YAML parse of `data/ontology/pipeline_stage_contracts.yaml`, `data/ontology/identity_indexes.yaml`, `data/ontology/property_indexes.yaml`, `data/ontology/node_schemas.yaml`, `scripts/governor/checks/adr_rules.yaml`; `git show 3d813db9e -- README.md AGENTS.md PRODUCT.md OKF.md .docker/README.md` to isolate the figures the wave-2 edits actually introduced; read-only grep of the fixed docs and the three wave-1 reports
- **Observed**:
  - Sources: `stages` = 19 (pipeline_stage_contracts.yaml:3, list of 19 entries); `indexes` = 46 (identity_indexes.yaml:2) + 10 (property_indexes.yaml:2) = 56; `node_schemas` = 43 (node_schemas.yaml:12); `rules` = 65 (adr_rules.yaml:8). ADR files ADR-037 … ADR-080 all exist under `doc/adr/` (44 ADR files + ADR-INDEX.md; index ids run 001–080 with 037–080 present).
  - README.md figures introduced by 3d813db9e all match: "43 node schemas, 56 indexes" (README.md:25, 469, 532), "19 stages" with a stage list whose order matches the YAML ids exactly — acquisition, catalog, parse, figure context, figure analysis, table fidelity, equation extraction, revision materialization, extraction compute and materialization, candidate materialization, embedding, summary, readiness, cluster, resolution, assurance, checkpoint, query (README.md:72-76), ADR range "037–080" (README.md:10, 467), binding `paper-summary-generate-default` → `agnes-ai/agnes-2.5-flash` (README.md:132-133 vs models.yaml:122-123 + models.yaml:40,43) and quality tier `glm/glm-4.5-air` / `summary-roles-v0.2` (README.md:134 vs models.yaml:47-54 — v0.2 is correctly attached to the glm quality tier, not to the agnes default, whose policy is summary-roles-v0.1 at models.yaml:45).
  - AGENTS.md introduced figure: "Reverse-ADR audit enforces 65 rules" (AGENTS.md:285) matches adr_rules.yaml:8 (65). PRODUCT.md introduced figures: "56 indexes … 43 schemas" (PRODUCT.md:15) match the sources. OKF.md diff removed only the `logs/` tree line (no figures); doc/OKF.md:5 "ADR-037…080" matches the file range.
  - Wave-1 reports (doc/review/grobid-state.md, architecture-map.md, functionality-ledger.md) contain no numeric claim on stage/schema/index/ADR-rule/model-binding counts that contradicts the fixed docs; architecture-map.md:74 only notes ADR-042–049 are `Proposed`, which does not contradict the "Binding ADRs 037–080" pointer.
  - **Finding trigger**: AGENTS.md:348 still reads "# Export all 31 node schemas to YAML" — 31 contradicts its source (node_schemas.yaml:12 → 43 schemas) and the same wave's README.md:25/PRODUCT.md:15 "43". The line was not touched by 3d813db9e and is not among T004's remediation rows (`.project/tasks/T004-fix-doc-remediation.md:42-63` covers AGENTS.md only for the 7-rules and stash lines), but it sits in a fixed doc and contradicts that doc's source, which the risk text defines as a finding.
- **Reference**: file:line pairs above; diff evidence via `git show 3d813db9e`

## Finding

- **Found**: 1 contradiction. All figures the wave-2 edits introduced are correct against the sources and consistent with the wave-1 reports, but AGENTS.md:348 ("Export all 31 node schemas to YAML") contradicts `data/ontology/node_schemas.yaml:12` (43 schemas) and the corrected counts in README.md:25 and PRODUCT.md:15 — the doc set is internally inconsistent on exactly the figure class wave 2 was remediated to fix.
- **Fix direction**: extend the row-7-class correction to AGENTS.md:348 — change "31 node schemas" to "43 node schemas" (rendered from `data/ontology/node_schemas.yaml`), or add it as an explicit remediation row in T004 and re-run the row's stale-string check.
