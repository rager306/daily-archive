# M005/S01 — Import Model Review Rubric

## Review Goal

Review whether S01 gives S02-S06 a safe, concrete, import-ready chunk model boundary. The review should catch overclaims and missing contract fields before baseline measurement starts.

## Required Inputs

- `import-ready-chunk-contract.md`
- `gold-corpus-manifest.json`
- `gold-corpus-rationale.md`
- `chunk_import_contract.py`
- `test_chunk_import_contract.py`

## Verdicts

| Verdict | Meaning |
|---|---|
| `PASS` | S01 is safe for S02 baseline measurement. Minor notes may exist, but no blocker undermines the contract/corpus/validator boundary. |
| `FLAG` | S02 may proceed only if findings are explicitly tracked; contract is usable but has meaningful gaps. |
| `BLOCK` | S02 should not proceed because the contract, corpus, or validator could create false import-readiness confidence. |

## Review Questions

### 1. Import model completeness

- Does the contract define package, paper, conversion, element, chunk, annotation, evidence path, warning, source span, and diagnostics objects?
- Are stable IDs, source spans, parent-child lineage, route/state enums, allowed/excluded uses, and refusal reasons defined?
- Is package validity clearly separated from import eligibility?

### 2. Safety boundaries

- Does the contract block production KG writes in M005?
- Does it avoid claims about semantic/vector retrieval, DSPy, LLM chunking, or broad corpus readiness?
- Does it prevent annotations from being treated as KG facts?
- Does it forbid raw text, raw chunk text, embeddings, vectors, secrets, and optimizer traces in machine logs?

### 3. Validator meaningfulness

- Do tests verify meaningful contract failures rather than only schema construction?
- Are missing IDs, missing source spans, unresolved parent references, unresolved evidence paths, invalid import states, raw-text leakage, embedding/vector leakage, annotation fact promotion, and claim-route pollution covered?
- Does the validator allow a structurally valid retrieval-only package with zero import-eligible chunks without calling it import-ready?

### 4. Gold corpus coverage

- Does the manifest reuse the existing M004 ten-paper corpus rather than silently expanding scope?
- Does the inner review set cover repaired conversion failures, known chunk-review blocker evidence, S07 trusted candidates, math/theory, multimodal/table/figure risk, and method/result boundary risk?
- Are missing artifacts treated as measurement findings rather than silently skipped?

### 5. Overclaim and false-confidence checks

- Does S01 avoid claiming that import readiness is already proven?
- Does it avoid count-only validation?
- Are future slices required to produce real-paper metrics and independent review before import claims?

## Required Review Output

The review summary must include:

- `Verdict: PASS|FLAG|BLOCK`
- Scope reviewed
- Findings grouped as blockers, flags, and notes
- Explicit overclaim assessment
- Explicit raw-text/embedding leakage assessment
- Explicit corpus coverage assessment
- Recommendation for whether S02 may proceed
