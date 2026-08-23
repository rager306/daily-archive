# M167 Write Path Classification

## Verdict

**Item 1 status: CLOSED for classification scope, migration follow-ups remain.**

M167 created a deterministic AST inventory and classified write paths into actionable groups. This does not claim every writer is already atomic or run-scoped; it gives the project the missing map needed for future remediation.

## Inventory evidence

- JSON: `data/architecture-assessment/m167-write-path-inventory.json`
- Markdown: `data/architecture-assessment/m167-write-path-inventory.md`
- Drilldown: `.gsd/exec/e9c233d6-0cbf-4e17-90e6-a7deba230a38.stdout`

Summary:

| Category | Count | Interpretation |
|---|---:|---|
| script-only | 263 | Process-boundary scripts; not production package runtime unless imported by workflows/packages. |
| run-scoped | 41 | Output/artifact paths likely scoped by caller/date/run/output dir. |
| unknown | 26 | Needs manual ownership classification. |
| append-log | 7 | Diagnostics/event JSONL or log-like writes. |
| shared-state | 6 | Stable catalog/index/state-like paths; highest write-safety priority. |
| database | 1 | SQLite queue path. |
| total | 344 | AST-level write records. |

## Shared-state candidates

These need P1/P2 follow-up review for atomic write, lock, or single-writer assumptions:

| Path | Line | Target | Priority | Note |
|---|---:|---|---|---|
| `src/research_graph/application/validation/batch_state.py` | 252 | `output_path` | P2 | Validation state output; likely caller-scoped but named like shared state. |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py` | 540 | `summary_path` | P2 | Summary output; likely artifact-scoped. |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py` | 526 | `article_path` | P1 | Canonical catalog article write. |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py` | 550 | `index_path` | P1 | Canonical catalog index write. |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py` | 910 | `report_path` | P2 | Report output. |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 183 | `index_path` | P2 | Baseline measurement index artifact. |

Highest priority: canonical catalog article/index writes. They are durable shared artifacts and should be reviewed for atomic replacement or single-writer contract.

## Database candidate

| Path | Line | Target | Priority | Note |
|---|---:|---|---|---|
| `src/research_graph/workflows/universal_kb/queue.py` | 120 | `self.db_path` | P1 | SQLite queue state; handled by S04/S05 concurrency review/probe. |

## Append-log candidates

Append/log-like writes are mostly diagnostics JSONL or event files. They should be either run-scoped or single-writer. No immediate code change in M167 unless S05 queue work surfaces a direct defect.

Examples:

- article artifact diagnostics,
- graph readiness event paths,
- chunking/source asset diagnostics,
- chunk baseline diagnostics.

## Unknown candidates

There are 26 unknown source records. Manual review shows several are likely run-scoped outputs, caches, or stable helper writes:

- `src/research_graph/cli/__init__.py` includes session, state temp, and daily artifact writes. `write_state_json()` was made atomic in M164; remaining daily artifacts are run/date scoped.
- `markdown_converter.py` writes cache markdown/method files.
- graph readiness and quality modules write caller-provided artifact paths.
- workflow smoke/rehearsal modules write caller-provided paths.

Unknown does not mean unsafe; it means the scanner cannot infer ownership without local semantics.

## Classification rules going forward

1. **shared-state**: stable catalog/index/queue/global state path. Must be atomic, locked, or single-writer.
2. **run-scoped**: unique output directory or date/run-specific artifact. Prefer no shared filenames.
3. **append-log**: JSONL/events/diagnostics. Must be run-scoped or single-writer; concurrent append needs lock.
4. **database**: use database transaction/lease semantics; review separately.
5. **script-only**: acceptable as process-boundary code unless imported by packages/workflows.
6. **unknown**: must not be used as proof of safety until manually classified.

## P1 follow-ups

1. Review canonical catalog writes in `catalog_ingest.py` for atomic replacement or explicit single-writer process contract.
2. Review UniversalKBQueue database concurrency in S04/S05.
3. Add a future ratchet so new package-level write paths must be categorized in the inventory output.

## P2 follow-ups

1. Manually reduce the 26 unknown source records by improving scanner heuristics or adding local comments/policies.
2. Classify append-log paths as run-scoped or single-writer.
3. Decide whether cache writes (`markdown_converter.py`) need atomic replacement or are acceptable best-effort cache writes.

## Scanner limits

The scanner is AST-based and intentionally conservative. It does not track variable definitions, path lifetimes, lock ownership, transaction boundaries, or process topology. Its value is reproducible inventory and first-pass categories, not a proof of write safety.
