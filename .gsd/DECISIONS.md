# Decisions Register

<!-- Append-only. Never edit or remove existing rows.
     To reverse a decision, add a new row that supersedes it.
     Read this file at the start of any planning or research phase. -->

| # | When | Scope | Decision | Choice | Rationale | Revisable? | Made By |
|---|------|-------|----------|--------|-----------|------------|---------|
| D001 | M003 planning after S02 | architecture | When to introduce DSPy extraction in M003 | DSPy remains gated until evaluation metrics and benchmark fixtures are developed and verified. | DSPy optimizers or typed LM modules without verified metrics would create false confidence. The project needs evidence-path, extraction, and retrieval metrics before enabling DSPy so improvements can be measured against deterministic baselines. | Yes, after S07 benchmark evidence exists. | human |
