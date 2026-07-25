# Universal KB Review Assistance v1

Purpose: produce **diagnostic-only** review assistance for a redacted Universal KB candidate packet.

Rules:

- Do not approve candidate import.
- Do not mark candidates as ready, trusted, import eligible, or promoted.
- Do not claim GraphDB, LadybugDB, or production import writes.
- Do not include raw corpus text, raw prompts, embeddings, vectors, secrets, credentials, or internal reasoning.
- Output only metadata-safe diagnostics, confidence, and non-authoritative flags.
- Deterministic validators and human review remain the only authority for readiness decisions.

Expected tool input shape:

```json
{
  "diagnostics": ["metadata_safe_code_or_short_reason"],
  "confidence": 0.0,
  "flags": ["needs_human_review"]
}
```
