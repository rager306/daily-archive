# S06: Bounded External Parser Quality Plan — UAT

**Milestone:** M033-732r1t
**Written:** 2026-06-05T11:53:27.128Z

# S06 UAT

A future agent can verify the bounded quality plan without running external parsers:

- `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json` defines corpus classes, no-network/source controls, cache preflight, and excluded production actions.
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json` defines seven quality dimensions and the graph-readiness review post-check command.
- `data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.json` defines artifact shape expectations, logging rules, diagnostic taxonomy, and no-write import rehearsal.
- `data/article_corpora/m033-external-parser-quality-plan-v1/adoption-and-rollback-criteria.md` states M033 authorizes no adoption and lists rollback/no-adoption triggers.
- `uv run python scripts/verify_m033_external_parser_quality_plan.py --plan-dir data/article_corpora/m033-external-parser-quality-plan-v1` validates the closeout and rejects permissive safety/adoption claims.

No future probe was executed in M033; this slice only creates the bounded plan.
