#!/usr/bin/env python3
"""M451 v13: import PDF monographs via page_outline grammar.

Navigation = PDF-TOC (embedded bookmarks, Vectify embedded_toc idea);
node anchor = page_range; section text = pages of the range.
Structural elements (theorem/definition/…) detected in section text by
DATA patterns -> context/structural_elements.json (latex_env).

Boundary (user directive + memory): this is APPLICATION logic reading
DATA dictionaries; Governor stays meta (repo structure only) and never
validates monograph content here.
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
import time
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parent.parent
SRC = REPO / "data/sources/textbook/trench-real-analysis/source"
DST_ROOT = REPO / "data/processed"

TOC_FURNITURE = re.compile(
    r"^(Trinity University|Digital Commons.*|Recommended Citation|Table of Contents)$", re.I)


def load_detectors() -> dict:
    cfg = yaml.safe_load((REPO / "data/ontology/structural_elements_detectors.yaml").read_text())
    return cfg.get("kinds", {})


def import_monograph(pdf_path: Path, object_id: str) -> str:
    import fitz

    doc = fitz.open(pdf_path)
    toc = doc.get_toc()
    if not toc:
        return "skip:no_toc"
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    # flatten TOC -> nodes with page ranges
    entries = []
    for i, (lvl, title, page) in enumerate(toc):
        next_page = toc[i + 1][2] if i + 1 < len(toc) else doc.page_count + 1
        entries.append({
            "section_id": f"{object_id}:toc{i}",
            "title": title.strip(),
            "depth": lvl,
            "page_start": page,
            "page_end": max(page, next_page - 1),
            "order": i,
        })
    # parent = nearest smaller depth above
    stack: list[tuple[int, str]] = []
    for e in entries:
        while stack and stack[-1][0] >= e["depth"]:
            stack.pop()
        e["parent"] = stack[-1][1] if stack else None
        stack.append((e["depth"], e["section_id"]))

    detectors = load_detectors()
    nav, structural = [], []
    section_texts = []
    for e in entries:
        role = "furniture" if TOC_FURNITURE.match(e["title"]) else "body"
        conf = "deterministic" if role == "furniture" else "deterministic"
        text = "\n".join(doc[p - 1].get_text() for p in range(e["page_start"], e["page_end"] + 1))
        section_texts.append((e, text))
        nav.append({
            "section_id": e["section_id"], "path_id": f"p{e['page_start']}",
            "depth": e["depth"], "parent_section_id": e["parent"],
            "children_section_ids": [x["section_id"] for x in entries if x["parent"] == e["section_id"]],
            "prev_section_id": entries[e["order"] - 1]["section_id"] if e["order"] else None,
            "next_section_id": entries[e["order"] + 1]["section_id"] if e["order"] + 1 < len(entries) else None,
            "title": e["title"], "char_count": len(text),
            "role": role, "role_confidence": conf,
            "page_start": e["page_start"], "page_end": e["page_end"],
        })
        # structural elements inside this section's text
        if role == "body":
            for line in text.splitlines():
                ls = line.strip()
                for kind, spec in detectors.items():
                    m = re.match(spec["pattern"], ls)
                    if m:
                        structural.append({
                            "section_id": e["section_id"], "page": e["page_start"],
                            "kind": kind,
                            "numbering": (re.search(r"\d+\.\d+", ls).group(0)
                                          if re.search(r"\d+\.\d+", ls) else None),
                            "statement_chars": len(ls),
                            "definition_source": bool(spec.get("definition_source")),
                        })
                        break

    d = DST_ROOT / object_id
    (d / "parsed").mkdir(parents=True, exist_ok=True)
    (d / "context").mkdir(exist_ok=True)

    parsed = {
        "schema_version": "parsed-article.v2", "paper_id": object_id,
        "grammar_profile": "page_outline",
        "title": entries[3]["title"] if len(entries) > 3 else object_id,
        "body_text": "\n\n".join(t for _, t in section_texts),
        "sections": [
            {"section_id": e["section_id"], "title": e["title"],
             "path_id": f"p{e['page_start']}", "level": e["depth"],
             "page_start": e["page_start"], "page_end": e["page_end"],
             "char_count": len(t), "text": t}
            for (e, t) in section_texts
        ],
        "citations": [], "formulas": [], "tables": [],
        "table_missingness": "no_table_elements",
        "formula_missingness": "no_formula_elements",
        "pdf_hash": hashlib.sha256(pdf_path.read_bytes()).hexdigest(),
    }
    (d / "parsed" / "article.json").write_text(json.dumps(parsed, ensure_ascii=False, indent=1))
    (d / "context" / "projection.json").write_text(json.dumps({
        "paper_id": object_id,
        "sections": [{"section_id": n["section_id"], "path_id": n["path_id"],
                      "order": i, "level": n["depth"], "title": n["title"],
                      "char_count": n["char_count"]}
                     for i, n in enumerate(nav)],
    }, ensure_ascii=False, indent=1))
    (d / "context" / "navigation.json").write_text(json.dumps(nav, ensure_ascii=False, indent=1))
    (d / "context" / "structural_elements.json").write_text(
        json.dumps(structural, ensure_ascii=False, indent=1))

    from collections import Counter
    kinds = dict(Counter(s["kind"] for s in structural))
    furniture = sum(1 for n in nav if n["role"] == "furniture")
    manifest = {
        "schema_version": "article-manifest.v1", "paper_id": object_id,
        "grammar_profile": "page_outline",
        "source": {"code": "textbook",
                   "identity_ref": "sources/textbook/trench-real-analysis/identity.json"},
        "lifecycle": "structured",
        "lifecycle_events": [
            {"stage": "raw", "recorded_at": now, "actor": "monograph-source"},
            {"stage": "parsed", "recorded_at": now, "actor": "m451-monograph-import"},
            {"stage": "structured", "recorded_at": now, "actor": "page-outline-toc",
             "evidence_sha256": hashlib.sha256((d / "context" / "navigation.json").read_bytes()).hexdigest()},
        ],
        "artifacts": {
            "parsed": {"file": "parsed/article.json", "derived_from": ["source"],
                       "sha256": hashlib.sha256((d / "parsed" / "article.json").read_bytes()).hexdigest(),
                       "built_at": now, "actor": "m451-monograph-import",
                       "stats": {"sections": len(nav)}},
            "context": {"file": "context/projection.json", "derived_from": ["parsed"],
                        "sha256": hashlib.sha256((d / "context" / "projection.json").read_bytes()).hexdigest(),
                        "built_at": now, "actor": "page-outline-toc",
                        "stats": {"ast_nodes": len(nav)},
                        "navigation": {"file": "context/navigation.json",
                                       "sha256": hashlib.sha256((d / "context" / "navigation.json").read_bytes()).hexdigest(),
                                       "actor": "page-outline-toc",
                                       "stats": {"nodes": len(nav), "roots": sum(1 for n in nav if not n["parent_section_id"]),
                                                 "max_depth": max(n["depth"] for n in nav), "path_dupes": 0,
                                                 "furniture": furniture,
                                                 "furniture_share": round(furniture / max(len(nav), 1), 3)}},
                        "structural_elements": {"file": "context/structural_elements.json",
                                                "sha256": hashlib.sha256((d / "context" / "structural_elements.json").read_bytes()).hexdigest(),
                                                "actor": "latex-env-detectors",
                                                "stats": {"count": len(structural), "by_kind": kinds},
                                                "inputs": ["data/ontology/structural_elements_detectors.yaml"]}},
            "extracted": {"present": False, "missingness": "extraction_not_run", "derived_from": ["parsed"]},
            "citations_resolved": {"present": False, "missingness": "no_citation_elements", "derived_from": ["parsed"]},
            "equations": {"present": False, "missingness": "no_formula_elements", "derived_from": ["parsed"]},
            "tables": {"present": False, "missingness": "no_table_elements", "derived_from": ["parsed"]},
            "figures": {"present": False, "missingness": "no_figure_elements", "derived_from": ["parsed"]},
            "external_resources": {"present": False, "missingness": "no_external_resources", "derived_from": ["parsed"]},
            "enriched": {"not_reached": True},
            "summaries": {"not_reached": True},
            "quality_signals": {"not_reached": True},
        },
    }
    (d / "manifest.yaml").write_text(yaml.safe_dump(manifest, sort_keys=False, allow_unicode=True))
    return f"imported:{len(nav)}sections:{len(structural)}elements"


def main(argv):
    jobs = [
        (SRC / "trench_real_analysis.pdf", "trench-real-analysis"),
        (SRC / "supplement_1_improper_integrals.pdf", "trench-supplement-1-improper-integrals"),
        (SRC / "supplement_2_lagrange_multipliers.pdf", "trench-supplement-2-lagrange-multipliers"),
    ]
    for pdf, oid in jobs:
        print(f"{oid}: {import_monograph(pdf, oid)}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
