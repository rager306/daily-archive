# M186 Verifier Wave Outcomes

## Verdict

**Verifier wave produced two real application boundaries and one deliberate no-move.**

## Outcomes

| Slice | Target | Outcome | Boundary |
|---|---|---|---|
| S03 | M031 validation evidence path primitives | moved | `src/research_graph/application/validation/evidence_paths.py` owns reusable path safety primitives; M031 script keeps wrappers. |
| S05 | M025 catalog safety primitives | moved | `src/research_graph/application/corpus/catalog_safety.py` owns reusable catalog path and safety flag primitives; M025 script keeps wrappers. |
| S06 | M031 `build_evidence` | no-move | Remains script-local because it is M031-specific dossier assembly with one production caller and one test helper caller. |

## Guardrail state preserved

- `script-only=4`
- `unknown=0`
- `shared-state=0`
- M025 and M031 focused tests pass.
- Scoped M027 catalog tests pass with known baseline drift isolated to S16.

## Architectural effect

M186 improves the verifier boundary by extracting reusable primitives rather than moving milestone-specific orchestration. This matches the M185 wrapper pattern while avoiding speculative interfaces.
