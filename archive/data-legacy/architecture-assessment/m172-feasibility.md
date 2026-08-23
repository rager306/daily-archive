# M172 Feasibility

## Verdict

**Proceed with category expansion.**

Feasible scope is narrow: add path-family categories for reviewed evidence-producing subsystems only. Do not reduce risk by generic target-name matching.

## Why proceed

- Baseline inventory is green: `unknown=0`.
- Candidate groups are clustered by stable source paths.
- M171 already added focused fallback tests, so M172 can extend the same pattern.
- The likely implementation is a small classifier diff plus focused tests.

## Safety rules

1. Preserve `unknown=0`.
2. Preserve generic conservative fallback for unreviewed `state`, `index`, `catalog`, and queue-like targets.
3. Add categories only for exact reviewed path families.
4. Every new category needs at least one positive test and one fallback protection test.
5. If GitNexus cannot resolve scanner impact, document it and rely on final `detect_changes` plus tests.

## Feasible next target

Start with `graph-readiness-*` or similarly exact evidence family categories because those records cluster under `src/research_graph/infrastructure/graph/readiness/` and represent generated review/export/validation evidence, not shared mutable state.
