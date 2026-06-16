# M072 Source Selection

## Purpose

Select a small metadata-only train/validation source set for the reviewed extraction benchmark. This is not a production gold corpus; it is a first reviewed fixture set that exercises the M071 evaluator over real canonical catalog references.

## Selection basis

Sources were selected from `data/article_catalog/article_catalog/arxiv/*/*/article.json` entries with fetched metadata and canonical source refs. Labels are derived from article metadata fields such as title, category, and known project context. No raw article body text or PDF text is copied into fixtures.

## Safety boundary

- metadata-only: yes
- raw article text: no
- PDF text extraction: no
- external API calls: no
- graph writes: no
- fact promotion: no

## Train source refs

| Split | case_id | source_ref | article_json | basis |
|---|---|---|---|---|
| train | `case:train:2605.18211` | `artifact:catalog-arxiv-cs-cl-2605.18211` | `data/article_catalog/article_catalog/arxiv/cs-cl/2605.18211/article.json` | title metadata mentions Graph Structure, Seq2Seq, Knowledge Graph Link Prediction |
| train | `case:train:1206.6423` | `artifact:catalog-arxiv-cs-cl-1206.6423` | `data/article_catalog/article_catalog/arxiv/cs-cl/1206.6423/article.json` | title metadata mentions Language, Perception, Grounded Attribute Learning |
| train | `case:train:2507.19457` | `artifact:catalog-arxiv-cs-cl-2507.19457` | `data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/article.json` | title metadata mentions GEPA, Prompt Evolution, Reinforcement Learning |
| train | `case:train:1409.0473` | `artifact:catalog-arxiv-cs-cl-1409.0473` | `data/article_catalog/article_catalog/arxiv/cs-cl/1409.0473/article.json` | title metadata mentions Neural Machine Translation, Align, Translate |
| train | `case:train:2511.20639` | `artifact:catalog-arxiv-cs-cl-2511.20639` | `data/article_catalog/article_catalog/arxiv/cs-cl/2511.20639/article.json` | title metadata mentions Multi-Agent Systems |
| train | `case:train:2108.12409` | `artifact:catalog-arxiv-cs-cl-2108.12409` | `data/article_catalog/article_catalog/arxiv/cs-cl/2108.12409/article.json` | title metadata mentions Attention with Linear Biases, Length Extrapolation |

## Validation source refs

| Split | case_id | source_ref | article_json | basis |
|---|---|---|---|---|
| validation | `case:validation:1606.02447` | `artifact:catalog-arxiv-cs-cl-1606.02447` | `data/article_catalog/article_catalog/arxiv/cs-cl/1606.02447/article.json` | title metadata mentions Learning Language Games through Interaction |
| validation | `case:validation:1611.04230` | `artifact:catalog-arxiv-cs-cl-1611.04230` | `data/article_catalog/article_catalog/arxiv/cs-cl/1611.04230/article.json` | title metadata mentions Neural Machine Translation and Monolingual Corpora |
| validation | `case:validation:2109.10862` | `artifact:catalog-arxiv-cs-cl-2109.10862` | `data/article_catalog/article_catalog/arxiv/cs-cl/2109.10862/article.json` | title metadata mentions Finetuned Language Models and Zero-shot Learners |

## Limitations

- These are reviewed metadata fixtures, not full-paper extraction gold labels.
- Labels are title-derived and intentionally small.
- They can validate benchmark mechanics and queue diagnostics, but cannot support production extraction quality claims.
- A future milestone should add full-paper reviewed labels before DSPy/MiniMax optimization.
