# GEPA hub research — 2507.19457

## Hub bibliography (GROBID TEI / citations.json)

| Metric | Value |
|--------|--------|
| Citations in TEI | 80 |
| With arXiv id | 23 |
| With DOI | 6 |
| Cited arXiv in catalog | 23/23 |
| Cited arXiv ingested as Paper | 23 |

## Ingest (process-local, mock embedder)

| Metric | Value |
|--------|--------|
| PDFs found | 24 |
| Ingest OK | 24 |
| Ingest fail | 0 |
| Total citations parsed | 1332 |
| Reference nodes written | 1332 |
| CITES resolved to Paper | 419 |
| Snapshot | `artifacts/gepa_hub_research/hub_and_refs_research.sgsnap` |

## Graph inventory (M319 ontology-string honest counts)

| Metric | Value |
|--------|--------|
| Paper nodes | 24 |
| Reference nodes | 1255 |
| Section nodes | 85 |
| Category nodes | 1 |
| Citation nodes | 325 |
| Entity nodes | 0 |
| Source nodes | 1 |
| Total nodes | 1691 |
| CITES edges | 419 |
| hasPart edges | 2297 |
| FROM_SOURCE edges | 24 |
| inCategory edges | 1 |
| MENTIONS edges | 0 |
| Edge histogram | `{"FROM_SOURCE": 24, "inCategory": 1, "CITES": 419, "hasPart": 2297}` |

## Hub extraction (RuleBasedExtractor)

- Entities: **24**
- By type: `{"Method": 5, "Dataset": 5, "Model": 6, "Metric": 6, "Task": 2}`
- Sample labels: HotpotQA, LiveBench, MATH, GEPA, GRPO, RLVR, generalization, chain-of-thought reasoning, DROP, WMT, GPT-4, Qwen, BERT, GPT-4o, Gemini, LLaMA, F1, MSE, precision, accuracy

## Papers in graph

- `2212.14024`
- `2308.16369`
- `2310.11523`
- `2403.10704`
- `2408.11857`
- `2503.05592`
- `2503.09516`
- `2503.19470`
- `2504.12216`
- `2504.13818`
- `2504.14870`
- `2504.15777`
- `2505.07903`
- `2505.11595`
- `2505.12886`
- `2505.16483`
- `2505.17086`
- `2505.17447`
- `2505.18454`
- `2506.03487`
- `2506.05154`
- `2506.09942`
- `2506.20629`
- `2507.19457`

## Notes

- Research used **MockEmbedder** (FD_API returned 401 with current key).
- Structural graph (Paper/Reference/CITES/hasPart) is valid for connectivity research.
- Edge inventory matches ontology wire strings (`hasPart`, not Rust name `HAS_PART`).
- MENTIONS=0 expected: ingest does not write Entity MENTIONS (extract-only path).
- Vectors are **not** production TEI embeddings.
- `import_eligible=false` (D127).

## Reproduce

```bash
python3 scripts/download_arxiv_to_catalog.py --ids-file artifacts/gepa_hub_research/missing_arxiv.txt --skip-meta
cargo test -p da-adapters --lib real_tei -- --test-threads=1
cargo test -p da-domain --lib relation -- --test-threads=1
cargo run -p da-cli --example gepa_hub_research
```
