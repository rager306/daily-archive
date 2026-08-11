# ADR-058: Article Revision Expression Ontology

**Status:** Accepted
**Date:** 2026-08-11  
**Depends on:** ADR-037, ADR-043, ADR-046, ADR-050, ADR-053, ADR-056  
**Trigger:** live `da load https://arxiv.org/html/2506.07853v1` initially processed latest bytes; exact v1/v5 acquisition then proved materially different document structures under one arXiv work id.

## Context

arXiv identity and source revision are different concepts:

- `2506.07853` identifies one scholarly work;
- `2506.07853v1` and `2506.07853v5` identify immutable published expressions of that work;
- PDF and HTML are manifestations of one expression;
- sections, extracted evidence, summaries, claims, and embeddings are derived from a specific expression and must not be silently mixed across revisions.

The current production graph has versionless `Paper` and revision-agnostic Section VIDs (`vid:section:{paper}:{ordinal}`). Loading multiple revisions into one store would overwrite or alias revision-specific structure. `Source` is a provider registry node, not a document expression. `ArtifactVersion` is scoped to immutable process/code/config/model/container artifacts and is not a scholarly document expression.

## Decision

Adopt a three-level FRBR/FaBiO-aligned shape without changing canonical Paper identity:

```text
Paper                         # Work; vid:paper:2506.07853
  └─ HAS_REVISION → PaperRevision
       ├─ HAS_MANIFESTATION → SourceManifestation (pdf)
       ├─ HAS_MANIFESTATION → SourceManifestation (html)
       ├─ hasPart → revision-scoped Section
       └─ SUPERSEDES → previous PaperRevision
```

### PaperRevision

Immutable expression-level identity:

```text
vid = vid:paper-revision:arxiv:2506.07853:v5
paper_vid = vid:paper:2506.07853
source_code = arxiv
source_key = 2506.07853v5
revision = 5
is_current = true|false
published_at = provider timestamp when available
content_identity_sha256 = deterministic digest of selected canonical manifestation set
import_eligible = false by default
schema_version = 1
```

### SourceManifestation

Immutable content-addressed captured representation. Revision ownership is
represented by `HAS_MANIFESTATION`, not a single-valued property, because the
same bytes may legitimately be shared by multiple revisions:

```text
vid = vid:source-manifestation:<sha256>
format = pdf|html|tei|tex
sha256 = ...
bytes = ...
uri = requested provider URL
path = repo-relative catalog path
capture_status = captured_local
import_eligible = false by default
schema_version = 1
```

### Revision-scoped downstream identity

- Section VID: `vid:section:{paper}:{revision}:{ordinal}`.
- Extraction/evidence/cache lineage must include `revision_vid` and exact manifestation SHA.
- A work-level query may project the current revision explicitly; it must not infer “latest” from insertion order.
- `SUPERSEDES` links newer revision to the immediately previous observed revision. Missing intermediate revisions are allowed but explicit (`v5 SUPERSEDES v1` means observed predecessor, not proof that v2-v4 do not exist).
- Claims/entities remain shared semantic identities when appropriate, but evidence edges retain revision provenance.

## Non-decisions and locks

- Accepted by explicit user decision on 2026-08-11 (D223). Production ontology materialization is authorized only through the planned revision-aware use case and remains D127-locked.
- `Paper` VID remains versionless; no `Paper` per vN.
- Source files and load reports are provenance, not graph truth.
- D127 remains `import_eligible=false` until explicit user authorization.
- ADR-057 non-article ontology remains independent and Proposed.
- Do not repurpose `ArtifactVersion` or `Source` with incompatible semantics.

## Alternatives rejected

1. **One Paper per vN** — duplicates work identity and breaks enrichment/citation joins.
2. **Overwrite Paper/Section with latest** — destroys historical evidence and makes replay non-reproducible.
3. **Store revision only as a Paper property** — cannot represent simultaneous history or revision-scoped Sections.
4. **Reuse ArtifactVersion** — conflates scholarly expressions with research-process implementation artifacts.
5. **Only catalog metadata, no graph revision node** — adequate for acquisition but cannot answer revision-aware graph queries.

## Acceptance evidence required

Acceptance implementation evidence:

1. user explicitly accepted the ontology choice (D223);
2. YAML schemas and edge contracts pass registry/endpoint audits;
3. VID functions have unit/property/serde tests and do not alter `paper_vid`;
4. v1 and v5 coexist in one process-local Samyama graph with one Paper, two PaperRevision nodes, distinct Sections, manifestations, and cache lineage;
5. queries prove current projection and as-of/revision selection;
6. replay writes zero duplicate nodes/edges;
7. D127 remains false;
8. governor negative fixture blocks production `PaperRevision`/`SourceManifestation` before acceptance.

## Current product state

ADR-056 acquisition carries typed optional revision and selects exact `idvN` bytes. Versioned catalog metadata is stored separately (`article.vN.json`, `article.vN.html`, `source/idvN.pdf`). Until M375 revision-aware materialization and runtime proof complete, `da load` continues to materialize the existing versionless Paper graph in process-local/D127-locked mode; operators must not merge multiple revisions into one authoritative graph. Acceptance authorizes implementation, not an automatic D127 import unlock.
