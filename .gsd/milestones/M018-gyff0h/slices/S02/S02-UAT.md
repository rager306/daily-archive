# S02: ML package reachability map — UAT

**Milestone:** M018-gyff0h
**Written:** 2026-05-21T07:06:54.270Z

# S02 UAT

## Evidence

```text
direct_torch_imports_in_project_source=0
direct_transformers_imports_in_project_source=0
direct_docling_imports_in_project_source=1
active_cli_exposure_found=false
helper_or_script_exposure_found=true
production_kg_import_enabled=false
ladybugdb_write_enabled=false
```

## Runtime path

```text
acquire_sources_for_manifest -> MDConverter.convert -> _try_marker -> _try_docling -> DocumentConverter().convert(pdf_path)
```

## Verification

```text
m018-s02-reachability-guard-ok
```

