# M173 Feasibility

## Verdict

**Proceed with batch-two category expansion.**

The candidate groups are small, path-clustered, and testable. The implementation can remain a minimal exact-path classifier diff.

## Why proceed

- Baseline inventory is green: `unknown=0`.
- Candidate counts are small: 3 parser replay, 3 source scan, 2 graph probe.
- The candidate source paths are stable infrastructure modules, not broad target-name patterns.
- M172 already established the policy and tests for exact path-family category expansion.

## Safety rules

1. Add only exact source path-family rules.
2. Preserve generic fallback for unapproved `summary`, `destination`, `cache`, `profile`, `state`, `index`, `catalog`, and queue-like targets.
3. Every new category needs positive and fallback tests.
4. No scanner schema or traversal changes.
5. Pre-edit GitNexus impact must be attempted and documented.
