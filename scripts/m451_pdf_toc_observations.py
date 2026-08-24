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


def export(pdf: Path, object_id: str) -> Path:
    doc = fitz.open(pdf)
    toc = [{"level": lvl, "title": t, "page": p} for lvl, t, p in doc.get_toc()]
    pages = {p + 1: doc[p].get_text() for p in range(doc.page_count)}
    det = yaml.safe_load((REPO / "data/ontology/furniture_detectors.yaml").read_text())
    obs = {
        "toc": toc,
        "pages_text": pages,
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
