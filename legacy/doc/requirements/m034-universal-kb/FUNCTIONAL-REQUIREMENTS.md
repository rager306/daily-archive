# M034 Functional Requirements

## Generic Universal-KB Requirements

| ID | Requirement | Acceptance Criteria |
|---|---|---|
| FR-001 | The system must model knowledge sources as durable records. | A source record includes ID, type/domain, local path or bounded locator, source hash, and redaction/locality metadata. |
| FR-002 | The system must model processing as persisted jobs. | A job records stage, status, attempts, retry_after, input hash, output paths, and last_error_code. |
| FR-003 | The system must support lazy recomputation. | A downstream job is skipped when upstream artifact hash/tool/config state is fresh and marked stale when dependencies change. |
| FR-004 | The system must support dependency readiness. | A job cannot run until required source/artifact dependencies are ready and not stale or blocked. |
| FR-005 | The system must classify failures. | Failures are retryable, terminal, blocked, stale, or needs_review, with typed diagnostic codes. |
| FR-006 | The system must produce candidate packets before review. | Candidate packets reference evidence artifacts and never set import eligibility. |
| FR-007 | The system must produce review packets before readiness handoff. | Review packets include candidate refs, diagnostics, review state, and safety flags. |
| FR-008 | The system must expose status inspection. | A future CLI/report can show counts by status, blockers, stale artifacts, retry schedule, and safety flags. |
| FR-009 | The system must keep graph promotion explicit and future-scoped. | No artifact sets graph/write flags true without a future authorized graph-promotion milestone. |
| FR-010 | The system must preserve GraphDB portability. | Contracts use `KnowledgeSubstratePort` and do not hardcode final LadybugDB/FalkorDB/HelixDB write semantics. |

## Scientific-paper First-domain Requirements

| ID | Requirement | Acceptance Criteria |
|---|---|---|
| PFR-001 | Scientific papers must remain the primary first domain. | Paper-specific adapters map into generic source/job/artifact/candidate/review contracts. |
| PFR-002 | GROBID outputs must be scholarly sidecar candidates. | TEI/metadata/reference/citation/section outputs are candidate evidence only. |
| PFR-003 | OpenDataLoader outputs must be layout sidecar candidates. | Layout/OCR/table/coordinate outputs are candidate evidence only. |
| PFR-004 | Adaptix mappings must be structural adapters. | Adapter success validates shape/mapping only, not semantic truth or graph readiness. |
| PFR-005 | Paper review packets must preserve graph-readiness review boundaries. | Review packet output keeps import flags false until future authorization. |
| PFR-006 | Low-quality source/parser signals must be visible. | Diagnostics include low_quality_source, backend_unhealthy, model_cache_missing_no_network, adapter_mapping_failed, and review_packet_incomplete where applicable. |

## Safety Requirements

| ID | Requirement | Acceptance Criteria |
|---|---|---|
| SFR-001 | Parser, sidecar, adapter, and LLM outputs remain candidate evidence. | No parser or helper output can bypass candidate/review/readiness boundaries. |
| SFR-002 | Direct GraphDB writes are forbidden. | Verifiers fail if `graphdb_written=true` or backend-specific write flags are true before explicit authorization. |
| SFR-003 | Agentic orchestration is deferred. | Agent helpers are optional future workers only and never current orchestration authority. |
| SFR-004 | Safety defaults are explicit and false. | All pre-authorization artifacts keep `graph_import_allowed=false`, `graphdb_written=false`, `ladybugdb_written=false`, `production_import_attempted=false`, and `import_eligible=false`. |
