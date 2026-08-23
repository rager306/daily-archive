# M179 Impact Analysis

## Verdict

**Pre-edit GitNexus impact is UNKNOWN, not safety proof.**

The scanner and workflow targets did not resolve as graph symbols. This matches prior inventory milestones and must be compensated with focused tests, generated deltas, strict CI smoke, quality stack, and final `gitnexus_detect_changes`.

## Probes

| Target | Direction | Result |
|---|---|---|
| `_classify` | upstream | UNKNOWN: target not found |
| `scripts/inventory_write_paths.py` | upstream | UNKNOWN: target not found |
| `render_delta_markdown` | upstream | UNKNOWN: target not found |
| `architecture-guardrail.yml` | upstream | UNKNOWN: target not found |

## Risk handling

- Do not treat UNKNOWN as proof of safety.
- Keep scanner changes to exact source-path sets.
- Add focused tests for new categories and fallback script-only cases.
- Generate delta markdown from the M179 baseline.
- Run strict canonical baseline CI smoke.
- Run final GitNexus `detect_changes` after edits.
