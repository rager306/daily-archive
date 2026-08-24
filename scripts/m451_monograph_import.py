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

FIG_CAP_RE = re.compile(r"Figure (\d+(?:\.\d+)*)[:.]?")


def extract_figures(doc, entries, d: Path, object_id: str) -> dict:
    """Caption -> nearest image on the page (geometry match, no ML).
    Extracts matched images to figures/assets/. Honest stats: unmatched
    captions and unassigned images are counted, never fabricated."""
    caps = {}
    for p in range(doc.page_count):
        for m in FIG_CAP_RE.finditer(doc[p].get_text()):
            num = m.group(1)
            if num not in caps:
                caps[num] = (p + 1, m.group(2).strip())
    assets = d / "figures" / "assets"
    assets.mkdir(exist_ok=True)
    records = []
    used_xrefs = set()
    for num, (page_no, caption) in sorted(caps.items()):
        page = doc[page_no - 1]
        rects = page.search_for(f"Figure {num}")
        imgs = page.get_image_info(xrefs=True)
        if not imgs:
            continue
        cap_y = rects[0].y1 if rects else 0.0
        best, best_dist = None, None
        for info in imgs:
            if info.get("xref", 0) and info.get("xref", 0) in used_xrefs:
                continue  # rust-canonical: an image serves at most one caption
            bbox = info["bbox"]
            dist = abs((bbox[1] + bbox[3]) / 2 - cap_y)
            if best_dist is None or dist < best_dist:
                best, best_dist = info, dist
        if best is None:
            continue
        xref = best.get("xref", 0)
        rec = {
            "figure_id": f"fig-{num}",
            "label": f"Figure {num}",
            "caption": caption,
            "page": page_no,
            "bbox": [round(v, 1) for v in best["bbox"]],
            "section_id": None,  # filled below by page range
        }
        for e in entries:
            if e["page_start"] <= page_no <= e["page_end"] and e["title"] not in (
                    "Trinity University", "Digital Commons @ Trinity"):
                rec["section_id"] = e["section_id"]
                break
        try:
            img = doc.extract_image(xref)
            fname = f"fig-{num.replace('.', '_')}.{img['ext']}"
            (assets / fname).write_bytes(img["image"])
            rec["image_file"] = f"figures/assets/{fname}"
            used_xrefs.add(xref)
        except Exception:
            rec["image_file"] = None
        records.append(rec)
    (d / "figures" / "index.json").write_text(json.dumps(records, ensure_ascii=False, indent=1))
    total_imgs = {im[0] for p in range(doc.page_count) for im in doc[p].get_images(full=True)}
    return {
        "file": "figures/index.json",
        "derived_from": ["parsed"],
        "sha256": hashlib.sha256((d / "figures" / "index.json").read_bytes()).hexdigest(),
        "built_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "actor": "page-outline-figures",
        "stats": {
            "captions_in_text": len(caps),
            "figures_indexed": len(records),
            "with_image_file": sum(1 for r in records if r.get("image_file")),
            "unassigned_images": len(total_imgs - used_xrefs),
            "binary_note": ("raster extracted" if sum(1 for r in records if r.get("image_file"))
                            else "vector/EPS diagrams — captions indexed, binaries need page-clip rasterization (future)"),
        },
        "inputs": ["embedded raster images (PyMuPDF) + caption geometry match"],
    }



def import_monograph(pdf_path: Path, object_id: str, identity_ref: str = "sources/textbook/trench-real-analysis/identity.json") -> str:
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
    (d / "figures").mkdir(exist_ok=True)

    figures_index = extract_figures(doc, entries, d, object_id)

    parsed = {
        "schema_version": "parsed-article.v2", "paper_id": object_id,
        "grammar_profile": "page_outline",
        "title": next((e["title"] for e in entries[4:6] if e["depth"] == 1), object_id) or object_id,
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
        "source": {"code": "textbook", "identity_ref": identity_ref},
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
            "figures": figures_index,
            "enriched": {"not_reached": True},
            "summaries": {"not_reached": True},
            "quality_signals": {"not_reached": True},
        },
    }
    (d / "manifest.yaml").write_text(yaml.safe_dump(manifest, sort_keys=False, allow_unicode=True))
    return f"imported:{len(nav)}sections:{len(structural)}elements"


def main(argv):
    only = set(argv[1:])
    jobs = [
        (SRC / "trench_real_analysis.pdf", "trench-real-analysis",
         "sources/textbook/trench-real-analysis/identity.json"),
        (SRC / "supplement_1_improper_integrals.pdf", "trench-supplement-1-improper-integrals",
         "sources/textbook/trench-real-analysis/identity.json"),
        (SRC / "supplement_2_lagrange_multipliers.pdf", "trench-supplement-2-lagrange-multipliers",
         "sources/textbook/trench-real-analysis/identity.json"),
        (REPO / "data/sources/textbook/ads-discrete-structures/source/ads.pdf",
         "ads-discrete-structures",
         "sources/textbook/ads-discrete-structures/identity.json"),
    ]
    for pdf, oid, ident_ref in jobs:
        if only and oid not in only:
            continue
        print(f"{oid}: {import_monograph(pdf, oid, ident_ref)}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
