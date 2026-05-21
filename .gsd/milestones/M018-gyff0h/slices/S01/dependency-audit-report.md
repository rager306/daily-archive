# M018 S01 dependency audit report

## Scope

Sanitized vulnerability-audit summary for the current Python environment/dependency graph. This is evidence for triage, not a remediation or upgrade commit.

## Audit command

```text
uv run --with pip-audit pip-audit -f json --progress-spinner off
```

The raw JSON was used transiently to produce a sanitized summary and then removed. The committed artifact intentionally keeps package names, versions, advisory IDs, and counts only.

## Result

```text
vulnerable_dependency_count=2
total_vulnerability_count=19
```

| Package | Version | Finding count | Fix versions reported by pip-audit |
|---|---:|---:|---|
| torch | 2.12.0 | 11 | none |
| transformers | 5.8.1 | 8 | none |

## Non-vulnerable focus packages in this audit

```text
accelerate 1.13.0
docling 2.93.0
docling-ibm-models 3.13.2
torchvision 0.27.0
```

## Important interpretation

These findings are not direct M017 MiniMax helper findings. They are transitive ML-stack findings introduced through the existing `docling` runtime dependency path. Their practical severity depends on S02 reachability analysis: whether project code imports or executes the affected ML stack in active CLI/runtime paths and whether it processes untrusted paper/PDF/model inputs.

## Safety

- Dependencies changed: false.
- Raw advisory details embedded: false.
- Raw audit JSON persisted: false.
- Secrets logged: false.
- Raw corpus payload logged: false.
