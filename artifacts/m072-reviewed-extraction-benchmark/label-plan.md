# M072 Label Plan

## Purpose

Define the reviewed metadata labels for M072 train/validation fixtures. The goal is to exercise the executable benchmark gate, not to claim full scientific extraction quality.

## Labeling basis

Labels are hand-curated from metadata titles in canonical catalog `article.json` files. They are normalized labels suitable for fixture-level entity and relation metrics.

## Entity types used

| Type | Meaning | Example |
|---|---|---|
| `Method` | Named method, model family, or approach visible in metadata title | `GEPA`, `Attention with Linear Biases` |
| `Task` | Research task visible in metadata title | `Knowledge Graph Link Prediction`, `Neural Machine Translation` |
| `Dataset` | Dataset or corpus named in metadata title | `Monolingual Corpora` |
| `Metric` | Explicit metric named in metadata title | none in v1 fixtures |
| `Field` | Broad research field/category derived from title/category metadata | `Multi-Agent Systems`, `Grounded Attribute Learning` |

## Relation types used

| Relation | Meaning |
|---|---|
| `APPLIED_TO` | Method or approach applied to task/field |
| `USES_COMPONENT` | Method uses named component/technique |
| `CONTRASTS` | Baseline prediction intentionally uses wrong relation for partial case |

## Gold fixture strategy

- Each case has 2 entities and 1 relation.
- Evidence refs are synthetic metadata refs, e.g. `evidence:m072:train:2605.18211:method`.
- `schema_valid=true`, `json_valid=true`, and zero operational cost/latency are used for gold.

## Baseline prediction strategy

Baseline predictions are deterministic and intentionally imperfect:

- Some cases are perfect matches.
- Some cases miss one gold entity and add one plausible but wrong entity.
- Some cases use wrong relation type or wrong target.
- One validation case has an invalid schema flag or missing evidence to test diagnostic behavior.

This baseline is not a model output. It is a fixed fixture for metric mechanics.

## Reviewed status

`reviewed` means: labels were inspected and curated from metadata titles by the agent in this milestone. It does not mean human-reviewed full-paper extraction.

## Future upgrade path

Before DSPy/MiniMax optimization:

1. Expand labels beyond title metadata.
2. Add full-paper evidence paths from parser artifacts.
3. Add per-type F1 thresholds.
4. Add n-ary/hyperedge claim examples.
5. Add a human review pass for ambiguity.
