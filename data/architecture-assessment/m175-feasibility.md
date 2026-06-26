# M175 Feasibility

## Verdict

**Feasible with guardrails.** Options 1, 2, and 3 can run in one milestone because they touch the same inventory scanner surface and share the same baseline, but only if each direction remains independently reversible.

## Included directions

1. **Command-specific CLI outputs**: feasible for exact `src/research_graph/cli/__init__.py` path-family review.
2. **Remaining mixed broad outputs**: feasible as review plus only exact movements; no-code outcomes are acceptable when records are mixed.
3. **Inventory delta reporting**: feasible as a small reproducible report path; not a reporting framework.

## Non-negotiable constraints

- No generic target-name reclassification.
- No broad module-family movement without exact reviewed path evidence.
- `unknown=0` must remain true.
- `shared-state=0` must remain true.
- Existing precise categories from M171-M174 must not regress.
- Conservative buckets must remain visible when records are not reviewed.

## Execution shape

M175 keeps all three requested directions but sequences them in thin slices:

1. baseline;
2. candidate reviews;
3. scope decision;
4. impact and tests;
5. implementation in small independent chunks;
6. final inventory and generated delta;
7. integrated verification and quality.

## Risk control

If S02 or S03 finds that a candidate group is mixed, the milestone will record a no-code decision for that group instead of forcing a category. This keeps the user request moving while preserving D094-D096 exact path-family rules.
