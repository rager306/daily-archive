# M060-gakmo0 S02 Figure QA Judge Comparison

Generated: 2026-06-12T12:27:24.462774+00:00

## Safety

- Graph writes are not authorized.
- Production import is not authorized.
- Fact promotion is not authorized.
- External network default is disabled; live LLM calls use a diagnostic-only override.
- LLM calls default is disabled; override scope is M060-gakmo0 S02 only.
- Local diagnostic host reference: 127.0.0.1.

## Aggregate

- Figures judged: 30
- Category counts: {'data_plot': 15, 'schema_diagram': 15}
- Winner counts: {'figure-qa-judge-fast': 6, 'figure-qa-judge-quality': 23, 'tie': 1}
- Cost estimate: not measurable without an external MiniMax pricing table.

| Model | Caption accuracy | Figure completeness | Structural fidelity | Avg latency ms | Outliers | Failed |
|---|---:|---:|---:|---:|---:|---:|
| figure-qa-judge-fast (MiniMax-M2.7-highspeed) | 0.7477 | 0.7823 | 0.7467 | 23846.35 | 3 | 0 |
| figure-qa-judge-quality (MiniMax-M3) | 0.6907 | 0.8757 | 0.8603 | 8549.9 | 7 | 0 |

## Side-by-side

| Figure | Category | Winner | Δ caption | Δ completeness | Δ structural |
|---|---|---|---:|---:|---:|
| 1804.02767::1 | data_plot | figure-qa-judge-quality | -0.1 | 0.2 | 0.25 |
| 1804.02767::2 | data_plot | figure-qa-judge-quality | 0.05 | 0.15 | 0.15 |
| 1804.02767::4 | data_plot | figure-qa-judge-quality | 0.1 | 0.1 | 0.12 |
| 1804.02767::5 | data_plot | figure-qa-judge-quality | -0.15 | 0.15 | 0.1 |
| 2507.19457::1 | data_plot | figure-qa-judge-quality | -0.05 | 0.1 | 0.0 |
| 2507.19457::15 | data_plot | figure-qa-judge-quality | 0.07 | 0.15 | 0.2 |
| 2507.19457::16 | data_plot | figure-qa-judge-quality | -0.05 | 0.1 | 0.1 |
| 2507.19457::18 | data_plot | figure-qa-judge-quality | -0.05 | 0.0 | 0.1 |
| 2507.19457::19 | data_plot | figure-qa-judge-quality | -0.05 | 0.05 | 0.05 |
| 2601.05808::20 | data_plot | figure-qa-judge-quality | 0.15 | 0.1 | 0.15 |
| 2601.05808::21 | data_plot | figure-qa-judge-quality | 0.0 | 0.1 | 0.13 |
| 2601.05808::22 | data_plot | figure-qa-judge-fast | -0.15 | 0.05 | 0.05 |
| 2601.05808::23 | data_plot | figure-qa-judge-quality | -0.15 | 0.1 | 0.15 |
| 2601.05808::24 | data_plot | figure-qa-judge-fast | -0.25 | 0.05 | 0.15 |
| 2601.05808::26 | data_plot | figure-qa-judge-quality | 0.0 | 0.0 | 0.1 |
| 1804.02767::3 | schema_diagram | figure-qa-judge-quality | 0.05 | 0.1 | 0.18 |
| 2507.19457::2 | schema_diagram | figure-qa-judge-fast | -0.25 | -0.05 | -0.1 |
| 2507.19457::3 | schema_diagram | figure-qa-judge-quality | 0.05 | 0.15 | 0.05 |
| 2507.19457::4 | schema_diagram | figure-qa-judge-quality | 0.07 | 0.1 | 0.08 |
| 2507.19457::6 | schema_diagram | figure-qa-judge-quality | 0.1 | 0.15 | 0.3 |
| 2507.19457::7 | schema_diagram | figure-qa-judge-fast | -0.4 | 0.0 | -0.05 |
| 2507.19457::8 | schema_diagram | figure-qa-judge-quality | -0.2 | 0.35 | 0.3 |
| 2507.19457::9 | schema_diagram | figure-qa-judge-quality | -0.2 | 0.15 | 0.1 |
| 2507.19457::10 | schema_diagram | figure-qa-judge-quality | -0.05 | 0.15 | 0.05 |
| 2507.19457::11 | schema_diagram | figure-qa-judge-fast | -0.45 | -0.1 | -0.15 |
| 2507.19457::12 | schema_diagram | figure-qa-judge-fast | 0.1 | -0.3 | 0.0 |
| 2507.19457::13 | schema_diagram | figure-qa-judge-quality | 0.05 | 0.3 | 0.25 |
| 2507.19457::14 | schema_diagram | figure-qa-judge-quality | 0.1 | 0.2 | 0.25 |
| 2507.19457::20 | schema_diagram | figure-qa-judge-quality | 0.15 | 0.15 | 0.2 |
| 2507.19457::21 | schema_diagram | tie | -0.2 | 0.05 | 0.15 |
