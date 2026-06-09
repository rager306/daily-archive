---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Drafted the GraphDB deferral and no-direct-GraphDB-write ADRs.

Draft ADR-002 deferred GraphDB selection and ADR-005 no direct extractor/parser/sidecar to GraphDB path. Both must use the template, reference S01 audit findings, include R/D impact tables, and preserve safety non-authorization.

## Inputs

- `.gsd/milestones/M034-kuei9y/decision-package/ADR-TEMPLATE.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ADR-000-universal-kb-north-star.md`
- `.gsd/milestones/M034-kuei9y/decision-package/r-d-consistency-audit.json`

## Expected Output

- `.gsd/milestones/M034-kuei9y/decision-package/ADR-002-defer-final-graphdb-selection.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ADR-005-no-direct-extractor-to-graphdb-path.md`

## Verification

Run marker checks that ADR-002 and ADR-005 include required template sections, GraphDB candidates, safety flags, R/D references, and Mermaid diagram counts within limits.

## Observability Impact

GraphDB and write-boundary ADRs give future agents explicit non-authorization and comparison-gate surfaces.
