# M170 Cache Coordination Policy

## Verdict

**Use atomic-only coordination for the current M170 scope. Do not add lock-file or compare-and-swap code now.**

M169 already changed the relevant stable cache writes from direct writes to same-directory temp plus atomic replace. S04 found no concrete multi-writer bug in the four remaining shared-state records. For the same-key CLI/PDF cache cases, lock/CAS would add complexity without changing the current correctness contract enough to justify code.

## Current behavior reviewed

### CLI per-paper JSON artifacts

Source: `src/research_graph/cli/__init__.py`.

- `_write_text_atomic(path, text)` writes text to a sibling temp path and replaces the target.
- `write_paper_artifacts(scored)` writes:
  - `data/daily_analysis/papers/<paper_id>/paper.json`
  - `data/daily_analysis/papers/<paper_id>/scored.json`
- Existing focused test coverage includes same-date rerun overwrite behavior in `tests/test_analysis.py`.

Observed semantics:

- No partial final JSON file should be visible after a successful replace.
- Same-key concurrent writers are **last writer wins**.
- For the same paper and same scoring input, payload should be deterministic.
- If different scoring inputs race for the same paper, the final artifact reflects whichever run completed last; this is a run coordination concern, not a file write atomicity failure.

### PDF downloader cache

Source: `src/research_graph/infrastructure/corpus/ingestion/fetchers.py`.

- `PDFDownloader.download(...)` returns existing cached PDFs without network access.
- When missing, it downloads, validates content-type/signature, then calls `_atomic_write_bytes(pdf_path, response.content)`.
- `_atomic_write_bytes(...)` uses `tempfile.NamedTemporaryFile(..., dir=path.parent, delete=False)` and replaces the target.
- Existing focused test coverage asserts replacement reaches the final PDF target in `tests/test_pdf_downloader.py`.

Observed semantics:

- No partial final PDF file should be visible after a successful replace.
- Same-key concurrent missing-cache downloads may duplicate network work but converge on a valid final PDF.
- If the upstream serves different bytes for the same arXiv id concurrently, a lock would not solve semantic authority; checksum or provenance policy would be needed instead.

## Options considered

| Option | Pros | Cons | M170 decision |
|---|---|---|---|
| Atomic-only | Already implemented; prevents partial files; simple; stdlib-only; tests exist | Does not prevent duplicate same-key work; last-writer-wins | **Selected** |
| Lock-file | Serializes same-key writers; can avoid duplicate downloads | Adds stale lock cleanup, platform semantics, timeout policy, and failure modes; no proven bug | Deferred |
| Compare-and-swap style | Can detect stale overwrite or authority mismatch | Requires version/hash authority contract that current CLI/PDF cache does not expose; more complex tests | Deferred |

## Why lock-file is deferred

A lock is justified when there is a real same-key multi-writer activation path where duplicate work or last-writer-wins corrupts the correctness contract. Current evidence does not show that:

- CLI per-paper artifacts are deterministic by paper/scored payload and are produced as part of a local analysis run.
- PDF cache writes converge on the same canonical arXiv PDF bytes for an id in normal operation.
- M170 is not activating high-concurrency CLI/PDF cache writers.

## Why CAS is deferred

Compare-and-swap needs an authority field: expected prior hash, expected generation id, or source revision. Neither current stable cache path exposes that as part of the contract. Adding CAS now would invent an authority model without a consumer.

## Activation triggers for future lock/CAS

Add a new lock/CAS milestone if any of these become true:

1. multiple worker processes write `write_paper_artifacts(...)` for the same paper id in parallel with non-deterministic scoring inputs;
2. PDF download concurrency causes measurable duplicate network cost or corrupt final cache evidence;
3. a cache consumer requires stale-overwrite detection rather than last-writer-wins;
4. an authority hash or generation id becomes part of the CLI/PDF cache contract.

## Required M170 downstream closure

S06 and S07 should close as policy-only unless new focused tests in those slices reveal a concrete bug.

S08 should verify:

```text
- existing focused CLI and PDF downloader tests still pass;
- write-path inventory remains unknown=0;
- shared-state records remain visible;
- closeout states atomic-only residual risk and future triggers.
```

## Residual risk

Atomic-only prevents partial final files, not duplicate same-key work. This is acceptable for M170 because no current activation path requires exactly-once cache population for CLI/PDF artifacts.
