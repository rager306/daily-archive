# ADR-062: Figure Quote Policy Window (soft target + absolute ceiling)

- Status: accepted
- Date: 2026-08-20
- Deciders: user (window direction), agent (implementation)
- Related: ADR-060 (scientific figure understanding pipeline), M389–M393 figure
  schema identity work, M413 diagnostics

## Context

`citations[].quote` in the figure analysis candidate schema was capped at
`maxLength: 160`. Live soaks showed the model cannot reliably self-truncate to
a small static cap: it violated the limit even when the schema was declared in
the system message and the rule was stated in the prompt. Raising the cap to
another static number (1500) merely moved the guess. Figures differ in how
much verbatim text a faithful citation needs — a one-number cap is the wrong
shape.

## Decision

Replace the single cap with a **measured policy window**:

1. **Soft target** (`soft_target_chars`, DATA): guideline for the model.
   Exceeding it is legal, measured, and logged (`figure_quote_measured` info
   with `max_quote_chars`, `figure_quote_soft_target_exceeded` warn). It is
   never a rejection.
2. **Absolute ceiling** (`absolute_max_chars`, DATA): equals the JSON-schema
   `maxLength` in `prompt_tasks.yaml` (`figure-analysis-candidate.v1`) and all
   per-type schemas in `figure_schemas.yaml`. Beyond it, validation stays
   terminal — fail-closed semantics are unchanged.
3. **Measurement loop**: every analysis stage execution records per-figure
   quote stats in stage metadata (`max_quote_chars`, `citation_count`,
   `quote_soft_target_exceeded`). Corpus probes accumulate the distribution;
   tuning `soft_target_chars` (and, deliberately, the ceiling) is a DATA-only
   change. Policy identity (`quote_policy=soft=…,abs=…`) is part of the stage
   `configuration_fingerprint`, so policy edits advance cache identity instead
   of silently reusing stale analyses.

DATA lives in `data/ontology/figure_quote_policy.yaml`
(`DA_FIGURE_QUOTE_POLICY` env override for the path). Validation:
`1 <= soft <= absolute`, `absolute <= 8000` (storable-candidate bound).
Loaders: `da_adapters::load_figure_quote_policy`; type
`da_domain::figure_analysis::FigureQuotePolicy`.

## Consequences

- The prompt no longer states a number; it points at the schema ceiling. The
  single source of the numeric ceiling is the schema; the single source of the
  guideline is the policy file.
- Rejection semantics did not weaken: over-ceiling output is still a terminal
  `figure_analysis_contract` violation with full diagnostics (M413).
- Stored candidates may carry longer quotes than before; the ceiling bounds
  them. If corpus measurement shows the ceiling is too generous, lowering it is
  a DATA edit that intentionally invalidates prior cached analyses.
- D127 (import lock) is untouched: candidates remain non-authoritative.

## Compliance

- Governor pins `with_quote_policy`, both event names, and the policy DATA
  fields (`figure_quote_policy_window`, `figure_quote_policy_data`).
- Hexagonal placement: domain type → adapters loader → application stage
  enforcement/observability → CLI composition. No layer inversion.
