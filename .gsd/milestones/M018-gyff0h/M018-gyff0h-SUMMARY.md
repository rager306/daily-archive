---
id: M018-gyff0h
title: "ML Dependency Security Triage"
status: complete
completed_at: 2026-05-21T07:17:55.325Z
key_decisions:
  - R046 triage verdict: defer broad upgrade and isolate Docling fallback first.
  - No immediate main-CLI hotfix is required.
  - Torch/transformers risk is medium only when Docling fallback processes external PDFs; otherwise low/dormant.
key_files:
  - .gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-inventory.json
  - .gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-audit-summary.json
  - .gsd/milestones/M018-gyff0h/slices/S02/run-evidence/ml-reachability-map.json
  - .gsd/milestones/M018-gyff0h/slices/S03/run-evidence/final-dependency-security-guard.json
  - .gsd/milestones/M018-gyff0h/M018-gyff0h-VALIDATION.md
lessons_learned:
  - Transitive dependency CVEs need reachability analysis before severity claims.
  - Lazy fallback imports are still security-relevant when they process external files.
  - For ML stacks, isolation/gating can be safer than immediate broad upgrades when audit tools report no fix version.
---

# M018-gyff0h: ML Dependency Security Triage

**M018 classified ML dependency vulnerability debt and recommended gating Docling fallback before broad ML upgrades or source-acquisition runs.**

## What Happened

M018 triaged the dependency debt raised by M017. S01 showed the vulnerable packages are transitive through the direct `docling` dependency: torch 2.12.0 and transformers 5.8.1 have 19 audit findings total, with no fix versions reported by pip-audit. S02 mapped source reachability and found no direct torch/transformers imports, but did find lazy Docling fallback reachable from source acquisition helpers that can process external arXiv PDFs. S03 produced the final recommendation, received independent security PASS, and validated R046: defer broad upgrade, gate Docling fallback first, and keep KG/MiniMax safety blocks unchanged.

## Success Criteria Results

All milestone success criteria passed. The project now knows the vulnerable ML packages are fallback-reachable through Docling rather than direct main-path imports, and has a concrete follow-up: Docling fallback safety gate.

## Definition of Done Results

- [x] Dependency inventory captured.
- [x] Vulnerability audit summarized safely.
- [x] Reachability mapped to source/helper/runtime paths.
- [x] Independent security review passed.
- [x] R046 validated.
- [x] No dependency or production KG safety gate changed.

## Requirement Outcomes

R046 validated with inventory, audit, reachability, final guard, and independent security review evidence.

## Deviations

None. M018 intentionally did not perform dependency upgrades.

## Follow-ups

Plan and execute a Docling fallback safety gate milestone before new broad source-acquisition runs. Defer broad torch/transformers upgrade until after the gate and compatibility planning.
