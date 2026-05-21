# M018 S01 dependency inventory report

## Scope

Read-only inventory for ML dependency debt surfaced during M017 security review.

## Direct dependency source

`pyproject.toml` declares `docling>=2.93.0` as a direct runtime dependency. `torch`, `torchvision`, `transformers`, `accelerate`, `tokenizers`, and `safetensors` are not direct project dependencies.

## Installed and locked focus packages

| Package | Version | Direct project dependency | Focus dependency notes |
|---|---:|---|---|
| docling | 2.93.0 | yes | Direct runtime dependency |
| docling-core | 2.75.0 | no | Pulled by docling/docling-ibm-models paths |
| docling-ibm-models | 3.13.2 | no | Pulls accelerate, docling-core, safetensors, torch, torchvision, transformers |
| accelerate | 1.13.0 | no | Pulls torch and safetensors |
| torch | 2.12.0 | no | Pulled by docling-ibm-models, accelerate, torchvision |
| torchvision | 0.27.0 | no | Pulls torch |
| transformers | 5.8.1 | no | Pulled by docling-ibm-models; pulls tokenizers and safetensors |
| tokenizers | 0.22.2 | no | Pulled by transformers |
| safetensors | 0.7.0 | no | Pulled by accelerate, docling-ibm-models, transformers |

## Inferred dependency paths

```text
arxiv-daily-archive -> docling -> docling-ibm-models -> torch
arxiv-daily-archive -> docling -> docling-ibm-models -> transformers
arxiv-daily-archive -> docling -> docling-ibm-models -> torchvision -> torch
arxiv-daily-archive -> docling -> docling-ibm-models -> accelerate -> torch
```

## Commands captured

- `uv run python --version` -> Python 3.13.12
- `uv pip list --format json` -> 141 installed packages; focus package versions captured in `run-evidence/dependency-inventory.json`

## Safety

No dependency files were modified. No secrets, raw corpus payloads, embeddings, vectors, or model payloads were logged.
