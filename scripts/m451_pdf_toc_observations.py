#!/usr/bin/env python3
"""M451 phase-2: PDF TOC observations exporter (reference extractor).

Thin PyMuPDF dump: toc bookmarks, per-page text, total pages, sha256,
and furniture patterns from furniture_detectors.yaml (DATA). Feeds the
RUST pdf-toc stage; extraction stays reference until a Rust PDF
backend (pdf-inspector candidate) is wired.
"""
from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

import fitz
import yaml

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "artifacts/pipeline_cache/pdf_toc_observations"


def dehyphenate(text: str) -> str:
    """Join words broken across lines by PDF layout: 'associ-\nated' →
    'associated'. Standard reference-extractor preprocessing (algorithms-for-
    decision-making specimen: fragments like 'challenges associ-ated' leaked
    into entity labels; ads 645 + trench 320 occurrences corpus-wide)."""
    import re
    return re.sub(r"(\w)-\s*\n\s*(\w)", r"\1\2", text)


def classify_page_lines(page: fitz.Page) -> tuple[str, str]:
    """Layout-aware extraction for complex typesetting (Tufte-style margins).

    Line-level classification by geometry+font (algorithms-for-decision-making
    specimen: PyMuPDF blocks naively merge margin sidenotes into body text,
    destroying reading order — 'then we can [FOOTNOTE 4] show that P must…'):

    - header:    y0 < 50                     (running heads, page numbers)
    - footer:    y0 > H-70                   (copyright/timestamp lines)
    - margin:    x0 > 0.65*W, not numeric    (sidenotes; short numeric lines
                 are figure axis labels)
    - body:      everything else, reading order (y,x)

    Returns (body_text, margin_notes_text); both dehyphenated.
    """
    W, H = page.rect.width, page.rect.height
    d = page.get_text("dict")
    body, margin = [], []
    for b in d["blocks"]:
        if b["type"] != 0:
            continue
        for l in b["lines"]:
            x0, y0, x1, y1 = l["bbox"]
            txt = "".join(s["text"] for s in l["spans"]).strip()
            if not txt:
                continue
            if y0 < 50 or y0 > H - 70:
                continue  # header/footer furniture
            if x0 > 0.65 * W:
                # margin zone: sidenote continuation vs figure axis label
                stripped = txt.replace(".", "").replace(",", "").strip()
                if len(stripped) >= 4 and not stripped.replace("/", "").isdigit():
                    margin.append((y0, txt))
                continue  # figure labels dropped
            body.append((y0, x0, txt))
    body.sort()
    margin.sort()
    body_text = dehyphenate("\n".join(t for _, _, t in body))
    margin_text = dehyphenate("\n".join(t for _, t in margin))
    return body_text, margin_text


def export(pdf: Path, object_id: str) -> Path:
    doc = fitz.open(pdf)
    toc = [{"level": lvl, "title": t, "page": p} for lvl, t, p in doc.get_toc()]
    pages, margin_notes = {}, {}
    for p in range(doc.page_count):
        body, margin = classify_page_lines(doc[p])
        pages[p + 1] = body
        if margin:
            margin_notes[p + 1] = margin
    det = yaml.safe_load((REPO / "data/ontology/furniture_detectors.yaml").read_text())
    obs = {
        "toc": toc,
        "pages_text": pages,
        "pages_margin_notes": margin_notes,
        "total_pages": doc.page_count,
        "pdf_sha256": hashlib.sha256(pdf.read_bytes()).hexdigest(),
        "furniture_patterns": det.get("monograph_toc_furniture", {}).get("patterns", []),
    }
    OUT.mkdir(parents=True, exist_ok=True)
    out = OUT / f"{object_id}.json"
    out.write_text(json.dumps(obs, ensure_ascii=False))
    return out


def main(argv):
    targets = {
        "trench-real-analysis": REPO / "data/sources/textbook/trench-real-analysis/source/trench_real_analysis.pdf",
        "ads-discrete-structures": REPO / "data/sources/textbook/ads-discrete-structures/source/ads.pdf",
        "algorithms-for-decision-making": REPO / "data/sources/textbook/algorithms-for-decision-making/source/algorithms-for-decision-making.pdf",
    }
    for oid in (argv[1:] or targets):
        pdf = targets.get(oid)
        if not pdf or not pdf.exists():
            print(f"{oid}: no pdf target")
            continue
        out = export(pdf, oid)
        d = json.loads(out.read_text())
        print(f"{oid}: toc={len(d['toc'])} pages={d['total_pages']} -> {out.name}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
