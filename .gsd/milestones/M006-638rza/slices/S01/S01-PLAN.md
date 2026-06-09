# S01: S01

**Goal:** Select a deterministic 30-paper deviation-scan corpus and audit local availability of normalized Markdown, PDF, and research workspace artifacts before running 30-paper measurement.
**Demo:** After this slice, there is a 30-paper manifest with selection rationale, local source availability, and known risk tags.

## Must-Haves

- 30 unique paper ids selected deterministically from local project evidence/cache.
- M005 10-paper corpus is included for baseline overlap.
- Per-paper availability records Markdown/PDF/research workspace status.
- Selection roles and risk tags are recorded without raw paper text.
- Artifact guard confirms no raw text, embeddings, vectors, or production write flags.

## Proof Level

- This slice proves: Manifest/audit artifacts plus guard script confirming 30 unique paper ids, M005 overlap, and no production import/write flags.

## Integration Closure

Consumes the M005 10-paper manifest and local paper/cache artifacts. Produces the M006 corpus manifest and availability summary used by S02 dry-run evidence generation.

## Verification

- Adds per-paper source availability diagnostics so missing-source problems are distinguishable from chunking/import-model deviations.

## Tasks

- [x] **T01: Selected the 30-paper deviation-scan corpus with M005 overlap preserved.** `est:medium`
  Discover candidate paper ids from local artifacts, caches, and M005/M004 manifests. Also inspect local external artifact roots `/root/.research/papers` and `/root/.arxiv_cache` during execution, but record only redacted paths/status in outputs. Select 30 unique ids deterministically, preserving the M005 10-paper baseline overlap and adding 20 expansion papers from available local evidence. Record selection rationale and risk tags.
  - Files: `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`, `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-rationale.md`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
manifest=json.loads(Path('.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json').read_text())
ids=[p['paper_id'] for p in manifest['papers']]
assert len(ids)==30
assert len(set(ids))==30
assert manifest['production_import_attempted'] is False
assert manifest['ladybugdb_written'] is False
assert manifest['raw_text_included'] is False
print({'paper_count': len(ids), 'm005_overlap_count': manifest['m005_overlap_count']})
PY

- [x] **T02: Audited source availability and found the first major deviation: 20 expansion papers lack Markdown source artifacts.** `est:medium`
  Audit local availability for the 30 selected papers: normalized Markdown, original PDF, research workspace, and known derived artifacts. Summarize missing-source patterns separately from chunking/import-model issues. External filesystem roots may be inspected during execution but only redacted status/path metadata is written.
  - Files: `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json`, `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
summary=json.loads(Path('.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json').read_text())
assert summary['paper_count']==30
assert summary['raw_text_included'] is False
assert summary['production_import_attempted'] is False
assert Path('.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl').stat().st_size > 0
print(summary)
PY

- [x] **T03: Reported 30-paper corpus readiness and found source acquisition is required for a meaningful full scan.** `est:small`
  Write the S01 availability/rationale report, highlighting whether 30 papers are viable for S02, which missing-source gaps are expected blockers, and what deviation categories are likely to be interesting.
  - Files: `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md`
  - Verify: test -s .gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md && uv run python - <<'PY'
from pathlib import Path
text=Path('.gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md').read_text()
assert '30-paper' in text or 'thirty-paper' in text
assert 'M005 overlap' in text
print('report-ok')
PY

## Files Likely Touched

- .gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json
- .gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-rationale.md
- .gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json
- .gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl
- .gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md
