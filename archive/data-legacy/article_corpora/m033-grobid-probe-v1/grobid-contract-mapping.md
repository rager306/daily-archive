# M033 S02 GROBID Contract Mapping

## Verdict

`grobid-scholarly-sidecar-candidate`: GROBID CRF produced TEI for all three local PDFs and is a good candidate source for scholarly metadata, sections, bibliography, and citation markers. It is not graph-ready and does not authorize import or LadybugDB writes.

## Runtime finding

- Native build path requires OpenJDK 21+; local Java is 17, so Docker CRF was the bounded S02 path.
- CRF image proves service/API/TEI shape. Full/DL image remains a future accuracy comparison, especially for bibliography/citation quality.

## Coverage summary

- `papers_with_title`: 3/3
- `papers_with_abstract`: 3/3
- `papers_with_body_divs`: 3/3
- `papers_with_bibliography`: 3/3
- `papers_with_figures`: 3/3
- `papers_with_tables`: 3/3
- `papers_with_coordinates`: 3/3

## Mapping to daily-archive contracts

| Need | GROBID evidence | Mapping verdict | Safety boundary |
|---|---|---|---|
| SourceRef / provenance | local PDF path, TEI output path, request diagnostics | candidate source-sidecar only; still needs source hash/span adapter | no import eligibility |
| PageIndex / section hierarchy | TEI body div/head counts and section headings | promising for section nodes; requires stable anchors | no chunk readiness by default |
| SemanticChunk input | TEI paragraphs and sections | candidate text source only; reading order/body quality review required | parser-ready not implied |
| Bibliography/citations | TEI `listBibl`, `biblStruct`, `ref` markers | strong GROBID-specific value; full/DL may improve quality | no trusted fact promotion |
| Tables/figures/layout | TEI figures and optional coordinate attributes | partial; not equivalent to OpenDataLoader table/layout extraction | review-only candidates |
| Graph readiness | none | explicitly not provided | `graph_import_allowed=false` |

## Per-paper highlights

### 2507.19457
- title_present: True
- abstract_chars: 1349
- body_div_count: 69
- body_head_count: 85
- paragraph_count: 696
- bibliography_entry_count: 80
- citation_or_ref_marker_count: 156
- figure_count: 25
- table_like_figure_count: 3
- coordinate_attribute_count: 1

### 2512.24601
- title_present: True
- abstract_chars: 1138
- body_div_count: 52
- body_head_count: 63
- paragraph_count: 176
- bibliography_entry_count: 49
- citation_or_ref_marker_count: 139
- figure_count: 31
- table_like_figure_count: 2
- coordinate_attribute_count: 25

### 2605.26525v1
- title_present: True
- abstract_chars: 1841
- body_div_count: 63
- body_head_count: 116
- paragraph_count: 232
- bibliography_entry_count: 78
- citation_or_ref_marker_count: 201
- figure_count: 58
- table_like_figure_count: 3
- coordinate_attribute_count: 34

## Required false flags

- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `production_import_attempted=false`
- `import_eligible=false`
- `trusted_kg_import_allowed=false`
- `graph_write_attempted=false`
