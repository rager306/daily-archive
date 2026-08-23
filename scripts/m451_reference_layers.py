#!/usr/bin/env python3
"""M451 reference layer builder — hand-tuned for 1-2 pilot articles.

This is the REFERENCE implementation of layer extraction per
data/schemas/article_schema.yaml: equations from parsed.formulas,
citations_resolved as classification (not resolution!), typed
missingness for absent layers. What this script does by hand, the
Rust pipeline stages will do later (W5).

NOT a mass migrator: it runs on explicitly named paper_ids only.
"""
from __future__ import annotations

import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

import yaml

NOT_REACHED_LAYERS = ["enriched", "summaries", "quality_signals"]


def sha256_file(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def build(paper_id: str, root: Path = Path("data/processed")) -> None:
    article = root / paper_id
    parsed = json.loads((article / "parsed" / "article.json").read_text())
    manifest_path = article / "manifest.yaml"
    manifest = yaml.safe_load(manifest_path.read_text())

    now = datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
    artifacts = manifest["artifacts"]

    # ── equations layer: re-export formulas with section binding ──────
    formulas = parsed.get("formulas") or []
    if formulas:
        eq_dir = article / "equations"
        eq_dir.mkdir(exist_ok=True)
        equations = [
            {
                "formula_id": f["formula_id"],
                "section_path_id": f["section_path_id"],
                "section_ordinal": f["section_ordinal"],
                "label": f.get("label"),
                "representation": f.get("representation"),
                "source_digest": f.get("source_digest"),
                "missingness": f.get("missingness"),
            }
            for f in formulas
        ]
        (eq_dir / "index.json").write_text(json.dumps(equations, ensure_ascii=False, indent=1))
        grounded = sum(1 for e in equations if e["representation"])
        artifacts["equations"] = {
            "file": "equations/index.json",
            "derived_from": ["parsed"],
            "sha256": sha256_file(eq_dir / "index.json"),
            "built_at": now,
            "actor": "m451-reference-builder",
            "stats": {"count": len(equations), "grounded": grounded,
                      "ungrounded": len(equations) - grounded},
        }
    else:
        # parsed itself carries the typed reason (no_formula_elements)
        artifacts["equations"] = {
            "present": False,
            "missingness": parsed.get("formula_missingness") or "no_formula_elements",
            "derived_from": ["parsed"],
        }

    # ── citations_resolved: CLASSIFICATION, never fake resolution ────
    # resolved_state vocabulary:
    #   resolved_arxiv        — arxiv_id present in parsed citations
    #   resolved_doi          — doi present
    #   candidate_title       — no ID but title present (match vs
    #                           citation_styles/venue_registry later)
    #   raw_only              — only raw GROBID text
    raw_citations = parsed.get("citations") or []
    if not raw_citations:
        artifacts["citations_resolved"] = {
            "present": False,
            "missingness": "no_citation_elements",
            "derived_from": ["parsed"],
        }
        citations = []
    else:
        citations = []
    for i, c in enumerate(raw_citations):
        if c.get("arxiv_id"):
            state = "resolved_arxiv"
        elif c.get("doi"):
            state = "resolved_doi"
        elif c.get("title"):
            state = "candidate_title"
        else:
            state = "raw_only"
        citations.append({
            "index": i,
            "resolved_state": state,
            "arxiv_id": c.get("arxiv_id"),
            "doi": c.get("doi"),
            "title": c.get("title"),
            "anchors": c.get("anchors"),
            "raw_text": c.get("raw_text"),
        })
    if raw_citations:
        (article / "citations_resolved.json").write_text(
            json.dumps(citations, ensure_ascii=False, indent=1))
        from collections import Counter
        states = dict(Counter(c["resolved_state"] for c in citations))
        artifacts["citations_resolved"] = {
            "file": "citations_resolved.json",
            "derived_from": ["parsed"],   # enricher joins later (W5)
            "sha256": sha256_file(article / "citations_resolved.json"),
            "built_at": now,
            "actor": "m451-reference-builder",
            "stats": {"total": len(citations), **states},
            "inputs": ["data/ontology/citation_styles.yaml",
                       "data/ontology/venue_registry.yaml"],  # dictionaries for the future title-matching
        }

    # ── typed missingness for absent layers ───────────────────────────
    # tables: parsed carries the typed reason itself
    if not (parsed.get("tables") or []):
        artifacts["tables"] = {
            "present": False,
            "missingness": parsed.get("table_missingness") or "no_table_elements",
            "derived_from": ["parsed"],
        }
    else:
        tab_dir = article / "tables"
        tab_dir.mkdir(exist_ok=True)
        tables = [
            {
                "table_id": t["table_id"],
                "section_path_id": t["section_path_id"],
                "section_ordinal": t["section_ordinal"],
                "caption": t.get("caption"),
                "cells": t.get("cells"),
                "source_digest": t.get("source_digest"),
                "missingness": t.get("missingness"),
            }
            for t in parsed["tables"]
        ]
        (tab_dir / "index.json").write_text(json.dumps(tables, ensure_ascii=False, indent=1))
        artifacts["tables"] = {
            "file": "tables/index.json",
            "derived_from": ["parsed"],
            "sha256": sha256_file(tab_dir / "index.json"),
            "built_at": now,
            "actor": "m451-reference-builder",
            "stats": {"count": len(tables),
                      "cells": sum(len(t["cells"] or []) for t in tables)},
        }

    # figures: parser produces no figure elements at all
    artifacts["figures"] = {
        "present": False,
        "missingness": "no_figure_elements",
        "derived_from": ["parsed"],
    }

    # ── not-reached layers (lifecycle has not arrived yet) ────────────
    for layer in NOT_REACHED_LAYERS:
        artifacts[layer] = {"not_reached": True}

    manifest_path.write_text(yaml.safe_dump(manifest, sort_keys=False, allow_unicode=True))
    print(f"{paper_id}: formulas={len(formulas)} citations={len(raw_citations)}")


if __name__ == "__main__":
    for pid in sys.argv[1:]:
        build(pid)
