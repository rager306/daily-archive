# M011 semantic review target selection

## Policy

Select all M010 outlier papers plus the first three deterministic non-outlier controls by M010 scan rank.

## Counts

- Source milestone: `M010-06v9ke`
- Source batch: `m010-next-plus-ten-materialized`
- Target count: `10`
- Outlier targets: `7`
- Control targets: `3`
- Missing source hashes: `0`

## Target IDs

- `M011-S01-TARGET-01` `2001.00278v2` `outlier`
- `M011-S01-TARGET-02` `2001.00279v1` `outlier`
- `M011-S01-TARGET-03` `2001.00281v1` `outlier`
- `M011-S01-TARGET-04` `2001.01587v2` `outlier`
- `M011-S01-TARGET-05` `2001.02595v2` `outlier`
- `M011-S01-TARGET-06` `2001.04832v1` `outlier`
- `M011-S01-TARGET-07` `2405.08246v1` `outlier`
- `M011-S01-TARGET-08` `2001.00818v1` `control`
- `M011-S01-TARGET-09` `2001.02741v1` `control`
- `M011-S01-TARGET-10` `2002.05505v6` `control`

## Redaction boundary

The target manifest stores source paths, source SHA256 hashes, paper-level aggregate metrics, and review instructions only. It intentionally does not embed raw paper text, chunk text, claim text, embeddings, vectors, secrets, optimizer traces, binary payloads, or base64.

## Span limitation

M010 scan diagnostics are paper-level aggregate records. Chunk-level source spans are not available in the redacted scan artifacts, so M011 S01 uses paper-level source references. S02 must keep judgments categorical and redacted.
