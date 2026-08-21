# Changelog

### M434 Audit both surfaces and declare sha fields

- The audit-fields drift gate scanned only da-application/src — blind across
  the crate boundary. da-cli load_journey stamps `embedding_sha256` on Paper
  (the first-attach path beside add_vector) and the shared embedding_write
  helper stamps it on PaperRevision (the revision content node); neither
  schema declared it, and M432 declared it only on Entity. The extended
  audit (90 files across da-application + da-cli) immediately reported the
  Paper drift — red run closed by declarations on both schemas.
- The scanner also learned to ignore its own doc-comment examples (a
  docstring `create_node("Label")` / `set_node_property_*(…, "field", …)`
  was being reported as drift). Binary audit now reports OK scanning both
  surfaces; schema unit tests pin the two new declarations. D127 locked.

### M433 Multi-line field scanner

- The audit-fields drift gate was line-based: cargo fmt routinely splits
  `set_node_property_*` calls across lines, and the scanner missed exactly
  those fields (M432: 2 of 4 drifted fields invisible — a blind gate is a
  fake gate). The extraction now lives in a pure tested lib function
  `da_cli::audit_fields::extract_set_fields` scanning the block from each
  occurrence to the matching closing paren.
- The fixed scanner immediately exposed its own false-positive class: a
  dynamic property loop (field passed as a variable) has no string literal
  and an unbounded scan misattributed the next literal —
  `create_edge(…, "hasSummary")` was reported as a Summary property.
  Paren-bounding closes it: the field must be a string literal inside the
  call's argument list. Five fixtures (single-line, multi-line, chained,
  no-string, dynamic-loop regression); binary audit OK. D127 locked.

### M432 CI matches the swept test surface; schema drift closed

- CI listed a dead `-p da-graph` (clippy fails with unmatched package ID
  on a fresh runner), omitted da-mcp entirely (46 real-engine tests never
  ran in CI), and skipped da-cli/da-adapters integration suites incl.
  the ADR-061 cross-process contract. Fixed and synced with the verify
  script. Running the suites locally exposed real schema drift: 4 fields
  written by M418-M419 but never declared (Paper triage trio, Entity
  embedding_sha256) — declared, audit-fields OK. The figure-journey test
  pins DA_FIGURE_QUOTE_POLICY absolutely. D127 locked.

### M431 Honest direct property write on the adapter