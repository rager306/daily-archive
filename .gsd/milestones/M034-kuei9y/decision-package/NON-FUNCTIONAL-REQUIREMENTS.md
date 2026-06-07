# M034 Non-functional Requirements

| ID | Requirement | Acceptance Criteria |
|---|---|---|
| NFR-001 | Local-first operation | Normal pipeline execution uses local artifacts and explicit bounded network modes only. |
| NFR-002 | Reproducibility | Source hashes, input hashes, tool/config versions, and output paths are persisted for replay. |
| NFR-003 | Redaction and payload safety | Diagnostics expose IDs, counts, hashes, paths, statuses, and codes, not raw paper text, secrets, embeddings, or credentials. |
| NFR-004 | Observability | Job status, failure code, retry count, retry_after, backend/cache health, and safety flags are queryable. |
| NFR-005 | Resumability | A stopped or crashed process can resume pending/retryable jobs from persisted state. |
| NFR-006 | Bounded concurrency | Concurrency is limited per stage/sidecar so slow or expensive backends do not starve the pipeline. |
| NFR-007 | GraphDB portability | Contracts do not assume a final GraphDB and keep `KnowledgeSubstratePort` backend-neutral. |
| NFR-008 | Reviewability | Human and LLM readers can determine what evidence exists, what is blocked, what is stale, and what remains unreviewed. |
| NFR-009 | Fail-closed defaults | Safety flags default to false: `graph_import_allowed=false`, `graphdb_written=false`, `ladybugdb_written=false`, `production_import_attempted=false`, `import_eligible=false`. |
| NFR-010 | Mermaid/readability discipline | Architecture documents keep prose/tables authoritative and use bounded diagrams only to clarify. |

## Acceptance Summary

A future implementation milestone satisfies these NFRs only if verifiers can prove persisted state, redacted diagnostics, no-write safety, backend portability, and resumable job behavior. M034 documents these requirements but does not implement them.
