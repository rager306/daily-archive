# S03: Deviation and pattern analysis — UAT

**Milestone:** M006-638rza
**Written:** 2026-05-19T18:09:25.671Z

# S03: Deviation and pattern analysis — UAT

## Smoke Test

Run the slice verification command and confirm it prints 34 tests passed, ruff clean, and an artifact guard with:

- `paper_count=30`
- `chunk_count=4289`
- `outlier_count=11`
- `import_eligible_chunk_count=0`
- `safety_flags_false=true`

## Expected Result

S03 provides Markdown-based 30-paper deviation evidence and a human-readable comparison to M005 baseline.

## Key Findings

- Retrieval-only remains dominant but drops as a share compared with M005/S03.
- Method, figure, citation, claim, and table routes are more visible in the 30-paper scan.
- 11 papers are flagged as deterministic outliers.
- Import eligibility remains zero.

## Not Proven

- Positive KG import readiness.
- PDF/multimodal completeness.
- Entity/relation extraction quality.
- Semantic/vector retrieval quality.
