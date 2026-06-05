# S02: GROBID Scholarly Parsing Study — UAT

**Milestone:** M033-732r1t
**Written:** 2026-06-05T10:19:26.763Z

# S02 UAT

A future agent can verify the bounded GROBID study without rerunning Docker:

- `data/article_corpora/m033-grobid-probe-v1/grobid-runtime-readiness.json` records Docker CRF selection and native JDK21 requirement.
- `data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json` records `status: tei-probe-complete`, three papers, three successes, zero failures.
- `data/article_corpora/m033-grobid-probe-v1/per-paper/*/grobid.tei.xml` contains real GROBID TEI outputs for the three local PDFs.
- `data/article_corpora/m033-grobid-probe-v1/grobid-tei-quality-summary.json` records 3/3 coverage for title, abstract, body divs, bibliography, figures, tables, and coordinates.
- `data/article_corpora/m033-grobid-probe-v1/grobid-probe-verdict.json` records `grobid-scholarly-sidecar-candidate` and `candidate_only: true`.
- `uv run python scripts/verify_m033_grobid_probe.py --probe-dir data/article_corpora/m033-grobid-probe-v1` validates closeout and rejects permissive safety flags.

All outputs are research evidence only; they are not graph-ready or import-eligible.
