#!/usr/bin/env python3
"""R024 S02: convert 33 PDFs to text via pymupdf.

Reads selection.json, finds articles with source_kind=pdf_converted,
converts their PDFs to text using pymupdf, caches results in
data/r024-53-document-corpus-v1/pdf-text-cache/<article_key>.txt
"""

from __future__ import annotations

import json
import sys
from datetime import UTC, datetime
from pathlib import Path

import fitz  # pymupdf

REPO_ROOT = Path("/root/daily-archive")
OUT_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1"
SELECTION = OUT_DIR / "selection.json"
CACHE_DIR = OUT_DIR / "pdf-text-cache"
EVENTS_LOG = OUT_DIR / "pdf-conversion-events.jsonl"
SUMMARY = OUT_DIR / "pdf-conversion-summary.json"


def main() -> int:
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    sel = json.loads(SELECTION.read_text())
    articles = sel["articles"]

    events: list[dict[str, object]] = []
    converted: list[dict[str, object]] = []
    failed: list[dict[str, str]] = []

    for a in articles:
        if a.get("source_kind") != "pdf_converted":
            continue
        ref = a["article_ref"]
        key = a["article_key"]
        pdf_path = REPO_ROOT / a["pdf_path"]
        out_path = CACHE_DIR / f"{key}.txt"

        try:
            if not pdf_path.exists():
                failed.append({"article_ref": ref, "error": f"PDF not found: {pdf_path}"})
                events.append(
                    {
                        "event": "pdf_conversion_failed",
                        "timestamp": datetime.now(UTC).isoformat(),
                        "article_ref": ref,
                        "error": "PDF not found",
                    }
                )
                continue
            doc = fitz.open(pdf_path)
            text = "\n".join([str(p.get_text()) for p in doc])
            n_pages = len(doc)
            doc.close()
            if len(text) < 500:
                failed.append({"article_ref": ref, "error": f"text too short: {len(text)} chars"})
                events.append(
                    {
                        "event": "pdf_conversion_failed",
                        "timestamp": datetime.now(UTC).isoformat(),
                        "article_ref": ref,
                        "error": f"text too short: {len(text)} chars",
                    }
                )
                continue
            out_path.write_text(text, encoding="utf-8")
            converted.append(
                {
                    "article_ref": ref,
                    "article_key": key,
                    "n_pages": n_pages,
                    "char_count": len(text),
                    "text_cache_path": str(out_path.relative_to(REPO_ROOT)),
                    "pdf_path": str(pdf_path.relative_to(REPO_ROOT)),
                }
            )
            events.append(
                {
                    "event": "pdf_conversion_complete",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "article_ref": ref,
                    "n_pages": n_pages,
                    "char_count": len(text),
                    "network_fetch_attempted": False,
                }
            )
            print(f"  OK {ref}: pages={n_pages}, chars={len(text)}")
        except Exception as e:
            err = str(e)[:120]
            failed.append({"article_ref": ref, "error": err})
            events.append(
                {
                    "event": "pdf_conversion_failed",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "article_ref": ref,
                    "error": err,
                }
            )
            print(f"  FAIL {ref}: {err}")

    with open(EVENTS_LOG, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")

    summary = {
        "schema_version": "r024-53-document-pdf-conversion-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "pdf_library": "pymupdf 1.27.2.3",
        "total_attempted": len(converted) + len(failed),
        "converted_count": len(converted),
        "failed_count": len(failed),
        "network_fetch_attempted": False,
        "fail_closed": True,
    }
    SUMMARY.write_text(json.dumps(summary, indent=2))

    print(f"\nsummary: {len(converted)} converted, {len(failed)} failed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
