# M018 independent security review

## Verdict

`PASS`

The independent security review agrees that the M018 recommendation is justified:

```text
DEFER BROAD UPGRADE; ISOLATE DOCLING FALLBACK BEFORE NEW SOURCE-ACQUISITION RUNS
```

## Review summary

Overall posture:

```text
Medium only when bounded source-acquisition helpers are run on external PDFs; Low/Dormant otherwise.
```

Deferring a broad upgrade while first isolating/gating Docling fallback is reasonable because:

- `torch` and `transformers` are transitive dependencies, not direct project imports;
- project source has no direct `torch` or `transformers` imports;
- Docling is reached through a narrow lazy fallback path;
- the path is real when source acquisition processes external PDFs;
- `pip-audit` reported no fix versions;
- broad ML-stack upgrades are operationally risky and should be separated from the safety gate.

## Evidence checked

- `dependency-inventory.json`
- `dependency-audit-summary.json`
- `ml-reachability-map.json`
- `ml-reachability-report.md`
- `dependency-security-triage.md`
- `final-dependency-security-guard.json`
- targeted source files around `MDConverter`, `PDFDownloader`, and `acquire_sources_for_manifest`

## Security non-findings

The review found no persisted raw secrets, raw corpus text, raw PDFs, embeddings, vectors, model payloads, or raw audit payloads in scoped artifacts.

A bounded scan found:

```text
secret_pattern_hits=0
raw_audit_indicator_hits=0
raw_payload_marker_hits=0
```

Exception: one benign `%PDF-` literal appears in prose explaining PDF magic-byte validation.

## Final review conclusion

No immediate hotfix is required for the main CLI or MiniMax helper path. Before any new broad source-acquisition run that may invoke Docling fallback on external PDFs, add an explicit Docling fallback safety gate.
