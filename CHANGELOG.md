# Changelog

### M437 corpus-wide semantic graph rebuild (MEM679)

- Full gold-corpus semantic graph materialized for the first time:
  **173 papers -> artifacts/gold_corpus/corpus_graph.sgsnap**
  (21868 nodes / 51518 edges), sha256-bound AllWavesReport with
  `replay_zero_writes` on every wave and a continuous graph chain —
  the ADR-051 §8.1 disk-to-query contract now proven at corpus scale.
- Process gap fixed (ADR-065): catalog is 100% versionless, so exact
  revision binding blocked the whole corpus. Added honest
  `tei_declared_revision` binding from the GROBID idno already stored in
  the parse cache (`arXiv:{id}v{N}`); fail-closed preserved when the
  document declares no version. 27 papers without any declared version
  are excluded via filtered DATA manifest (no silent denominator loss).
- Canaries: canary verification stage-id drift fixed (report-driven
  `extraction_compute` instead of hardcoded `extract` — red-first test);
  20 embedding-enabled waves over the shared cache root,
  `replay_all_cache_hit` everywhere, import_eligible=false throughout.


### M436 kg-pipeline honest execution engine

- Recon finding: `kg-pipeline::ExecutionEngine` was a fictitious contract —
  the doc-comment promised retry/failure policy/status tracking while the
  loop aborted on first error; `RetryPolicyRef` and `FailurePolicy` were
  never consumed; 4/6 `PipelineStatus` variants were unreachable. The
  product path never noticed because execution lives in
  `PipelineOrchestrator` (ADR-049) and kg-pipeline was used only for
  topology.
- Fixed (ADR-064): per-step retry from a policy registry with an injected
  `Sleeper` port (ADR-051 determinism infra — no real sleeps in tests);
  `FailurePolicy::Abort`/`Continue`/`RetryThenAbort` semantics with
  transitive skip of dependents; `attempts_per_step`/`skipped_steps` in
  `ExecutionResult`; observable FSM — `StepEvent` stream (Started/Retried/
  Failed/Skipped/Completed + StatusChanged Pending→Running→terminal) with
  all six statuses reachable; cooperative `EngineControl` pause/stop
  checked between steps.
- Red-first + mutation-checked: retry success on attempt 3, sleeper call
  count, Continue→abort mutation and pause-check mutation both caught by
  tests. `execute()`/`execute_with_policies()` keep historical single-
  attempt abort behavior (no consumer change; da-application
  acap_control_plane_test green).


### M435 Governor pins the audit gate

- M433/M434 made the audit-fields drift gate structurally correct
  (multi-line `extract_set_fields`, both product surfaces), but it only
  ran under CI's `schema_check_test` — in the dev loop a regression to
  the line-based scan or a single-surface scope would pass every commit
  and silently re-blind the ADR-045 gate.
- Two Governor pins (`audit_field_scanner` requiring `extract_set_fields`;
  `audit_both_surfaces` requiring `src_dirs` and forbidding the restored
  line-by-line `for bline in block.lines()` blind spot) now enforce it at
  every commit in ~1s with no build — the first line of defense.
- The pin construction itself exposed the scanner's doc-comment
  false-positive class once more: a naive `forbidden: line-by-line`
  tripped on the module doc's own description. The forbidden pattern is
  the exact regressed code shape, not the English phrase. D127 locked.

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