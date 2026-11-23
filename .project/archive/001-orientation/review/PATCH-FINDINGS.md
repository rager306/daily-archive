# Patch Findings

Reviewed HEAD: 7e891591a5a0da7131f61f4050bb8fd0c6dd09a3
State: ship/blocked

## Findings

### P001 — AGENTS.md still claims 31 node schemas against source 43

- **Source**: `.project/review/final-gap-2.md`
- **Locator**: Risk: cross-wave integration — every figure wave 2 (T004, landing 84dd6a1fa via workspace 3d813db9e) introduced into README.md/AGENTS.md/PRODUCT.md/OKF.md/doc/OKF.md must match the real ontology/governor sources and the wave-1 reports; any contradiction between a fixed doc, its source, and a wave-1 report claim is a finding.
- **Evidence**: 1 contradiction. All figures the wave-2 edits introduced are correct against the sources and consistent with the wave-1 reports, but AGENTS.md:348 ("Export all 31 node schemas to YAML") contradicts `data/ontology/node_schemas.yaml:12` (43 schemas) and the corrected counts in README.md:25 and PRODUCT.md:15 — the doc set is internally inconsistent on exactly the figure class wave 2 was remediated to fix.
- **Fix direction**: extend the row-7-class correction to AGENTS.md:348 — change "31 node schemas" to "43 node schemas" (rendered from `data/ontology/node_schemas.yaml`), or add it as an explicit remediation row in T004 and re-run the row's stale-string check.
