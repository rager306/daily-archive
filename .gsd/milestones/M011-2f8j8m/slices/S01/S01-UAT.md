# S01: Semantic review corpus selection — UAT

**Milestone:** M011-2f8j8m
**Written:** 2026-05-20T08:23:54.425Z

# S01: Semantic review corpus selection — UAT

## Expected

- Select bounded M010 semantic review targets.
- Include outliers and controls.
- Use source path/hash references.
- Do not embed raw paper text, chunk text, embeddings, vectors, secrets, optimizer traces, or binary payloads.

## Result

- Target count: `10`
- Outlier targets: `7`
- Control targets: `3`
- Source hash missing count: `0`
- Raw payload key count: `0`
- Safety flags false: `true`
- Production import attempted: `false`
- LadybugDB written: `false`

## Limitation

M010 redacted scan diagnostics are paper-level aggregate records, so S01 targets are paper-level source references, not chunk-span references.
