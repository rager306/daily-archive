# ADR-065: TEI-declared revision binding for versionless canonical assets

- Status: accepted
- Date: 2026-08-21
- Deciders: user (standing GSD mandate), agent (implementation)
- Related: ADR-053 (article revision model), M061 canonical-catalog
  normalization rule 1 (versionless identity), M437-p43udw (corpus graph
  rebuild), `revision_binding_from_catalog_checked`

## Context

The canonical article catalog stores versionless arXiv identities with bare
`{id}.pdf` assets and a bare `article.json`; versioned roots remain provenance
only when they carry distinct evidence. Corpus-wide graph materialization
(M437) requires an exact revision binding per source digest — the catalog
lookup resolves this from `article.v{N}.json` metadata or a unique versioned
asset, and fails closed otherwise ("no exact revision binding … is blocked").

Recon found the corpus is **100% versionless** (0/200 papers have versioned
metadata). The domain refuses to invent `v1`
(`versionless_does_not_invent_v1_context`), so the entire corpus wave was
blocked by design — correctly.

## Decision

Add one additional honest binding method, `tei_declared_revision`, applied
**only** when the checked catalog lookup reports the honest-versionless case:

1. The bare canonical asset must exist, its sha256 must equal the verified
   wave's `source_sha256`, and no versioned siblings may exist in the same
   `source/` directory (a versioned sibling makes the ambiguity unresolvable
   → fail closed).
2. The parse-cache payload (`ParsedArticle.tei_xml`) is scanned for the GROBID
   header idno `<idno type="arXiv">…</idno>`. GROBID writes e.g.
   `arXiv:1412.6980v9[cs.LG]`; the parser extracts `{paper_id}v{N}` with N>0.
3. If the document declares no version (or there is no TEI at all), the
   fallback returns "no evidence" and the original fail-closed error stands.

The revision is therefore **declared by the document itself**, never invented;
the binding method is recorded per paper in the materialization report
(`binding_method: tei_declared_revision`) so lineage stays auditable.

## Freshness and trust boundary

Derived evidence now carries a bounded `FreshnessIdentity` containing the
selected target revision, observed revision, source and representation
SHA-256 digests, parser identity, configuration identity, cache key, and the
observation timestamp. The timestamp is provenance only; no wall-clock oracle
is used. `assess_freshness` first requires the identity to target the selected
revision, then returns `fresh`, `stale`, or `superseded` from revision
identity alone. Consequently, equal digests do not make an observation valid
for a different revision.

`DeclaredRevisionTrust::TeiDeclared` is the sole authoritative declaration.
Any external comparison is represented as `DeferredCrossCheck` with its own
receipt reference and cannot overwrite the TEI claim. Missing fields,
non-SHA-256 digests, and target mismatches fail closed with typed errors.

## Consequences

- Corpus rebuild (M437): 173/200 gold papers bound via
  `tei_declared_revision`; the remaining 27 papers declare no version anywhere
  in their cached artifacts and are honestly excluded via a filtered DATA
  manifest (`versioned_corpus_manifest.json` records the excluded IDs) until
  upstream evidence exists. No silent denominator loss.
- `revision_binding_from_catalog` keeps its exact historical error text; the
  new `*_checked` variant returns a typed `RevisionBindingFailure` /
  `Ok(None)` instead of string-matching errors.
- Import remains locked (D127): all reports keep `import_eligible=false`;
  binding affects identity only, not admission.
- Replay safety is unchanged: full-corpus run proved `replay_zero_writes` on
  every wave with continuous graph chain (21868 nodes / 51518 edges).
