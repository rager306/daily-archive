#!/usr/bin/env python3
"""M451 phase-2: PDF page-observations exporter (reference extractor).

Thin, deterministic PyMuPDF dump: per-paper captions (Figure N.N rects),
raster images (xref+bbox), section page-ranges, extracted binaries.
Output feeds the RUST figures stage (analytic core) — extraction stays
here until a Rust PDF backend lands; the registry records the split.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import fitz

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "artifacts/pipeline_cache/pdf_observations"
FIG_CAP_RE = re.compile(r"Figure (\d+(?:\.\d+)*)[:.]?")


def export(pdf_path: Path, object_id: str) -> Path:
    doc = fitz.open(pdf_path)
    caps, images, files = [], [], {}
    seen = set()  # GLOBAL first-occurrence dedup (reference semantics)
    for p in range(doc.page_count):
        page = doc[p]
        text = page.get_text()
        for m in FIG_CAP_RE.finditer(text):
            num = m.group(1)
            if num in seen:
                continue
            seen.add(num)
            rects = page.search_for(f"Figure {num}")
            if rects:
                r = rects[0]
                caps.append({"label": f"Figure {num}", "page": p + 1,
                             "y0": round(r.y0, 2), "y1": round(r.y1, 2)})
        for info in page.get_image_info(xrefs=True):
            xref = info.get("xref", 0)
            if not xref:
                continue
            bb = info["bbox"]
            images.append({"xref": xref, "page": p + 1,
                           "bbox": [round(v, 2) for v in bb]})
            if xref not in files:
                try:
                    img = doc.extract_image(xref)
                    fname = f"xref-{xref}.{img['ext']}"
                    dstdir = REPO / f"data/processed/{object_id}/figures/assets"
                    dstdir.mkdir(parents=True, exist_ok=True)
                    (dstdir / fname).write_bytes(img["image"])
                    files[xref] = f"figures/assets/{fname}"
                except Exception:
                    pass
    obs = {"captions": caps, "images": images, "sections": [], "extracted_files": files}
    OUT.mkdir(parents=True, exist_ok=True)
    out = OUT / f"{object_id}.json"
    out.write_text(json.dumps(obs, ensure_ascii=False))
    return out


def main(argv):
    # jobs resolved from manifest source refs (monographs keep pdf path there)
    targets = {
        "ads-discrete-structures": REPO / "data/sources/textbook/ads-discrete-structures/source/ads.pdf",
        "trench-real-analysis": REPO / "data/sources/textbook/trench-real-analysis/source/trench_real_analysis.pdf",
    }
    for oid in (argv[1:] or targets):
        pdf = targets.get(oid)
        if pdf is None or not pdf.exists():
            print(f"{oid}: no pdf target"); continue
        out = export(pdf, oid)
        d = json.loads(out.read_text())
        print(f"{oid}: observations -> {out.name} (captions={len(d['captions'])}, images={len(d['images'])}, files={len(d['extracted_files'])})")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
