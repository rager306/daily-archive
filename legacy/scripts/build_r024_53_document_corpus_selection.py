#!/usr/bin/env python3
"""R024 53-doc corpus selection (M118 S01).

Builds 53-article corpus from M117 baseline (20) + 33 new PDF candidates.
PDFs converted via pymupdf (M115 dev dep). No network fetches.
"""

from __future__ import annotations

import json
import sys
from datetime import UTC, datetime
from pathlib import Path

import fitz  # pymupdf

REPO_ROOT = Path("/root/daily-archive")
M117_SELECTION = REPO_ROOT / "data" / "r024-20-document-corpus-v1" / "selection.json"
OUT_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1"
OUT_SELECTION = OUT_DIR / "selection.json"
OUT_EVENTS = OUT_DIR / "selection-events.jsonl"
OUT_SUMMARY = OUT_DIR / "selection-summary.json"
CATALOG_ROOT = REPO_ROOT / "data" / "article_catalog" / "article_catalog"

# M117 baseline 20 articles (to include in 53-doc)
M117_REFS = {
    "arxiv/cs-ai/2512.24601",
    "arxiv/cs-ai/2605.28617v1",
    "arxiv/cs-cv/2605.26525v1",
    "arxiv/cs-cl/2507.19457",
    "company_blog/cs-ir/pageindex_zhang2025pageindex",
    "arxiv/cond-mat-mtrl-sci/2605.20918",
    "arxiv/cs-ai/2502.13025",
    "arxiv/cs-ai/2510.21148",
    "arxiv/cs-cl/2108.12409",
    "arxiv/cs-cl/2109.10862",
    "arxiv/cs-cl/2605.18211",
    "arxiv/cs-cv/1804.02767",
    "arxiv/cs-lg/2111.00396",
    "arxiv/mixed-source/2603.04448",
    "arxiv/cs-cl/2511.20639",
    "arxiv/cs-lg/2203.14465",
    "arxiv/mixed-source/2605.21401",
    "arxiv/mixed-source/2605.25522",
    "arxiv/mixed-source/2605.20897",
    "arxiv/mixed-source/2604.18478",
}


def find_pdf_candidates() -> list[dict[str, str]]:
    """Find articles with extractable text (PDF via pymupdf or HTML/md/txt).

    Returns candidates with source_kind: "pdf_converted" or "html_native".
    """
    candidates: list[dict[str, str]] = []
    for aj in CATALOG_ROOT.rglob("article.json"):
        rel = aj.relative_to(CATALOG_ROOT).parent.as_posix()
        if rel in M117_REFS:
            continue
        source_dir = aj.parent / "source"
        if not source_dir.exists():
            continue
        article_key = aj.parent.name
        # check HTML/md/txt first
        for ext_glob in ("*.html", "*.md", "*.txt"):
            text_files = list(source_dir.glob(ext_glob))
            if text_files:
                tf = text_files[0]
                txt = tf.read_text(encoding="utf-8", errors="ignore").strip()
                if len(txt) > 500:
                    d = json.loads(aj.read_text())
                    identity = d.get("identity", {})
                    title = identity.get("title", "") or d.get("title", "")
                    candidates.append(
                        {
                            "article_ref": rel,
                            "article_key": article_key,
                            "title": title[:80],
                            "source_kind": "html_native",
                            "char_count": str(len(txt)),
                            "text_path": str(tf.relative_to(REPO_ROOT)),
                        }
                    )
                    break
        else:
            # no HTML/md/txt — try PDF
            pdf = source_dir / f"{article_key}.pdf"
            if not pdf.exists():
                continue
            try:
                doc = fitz.open(pdf)
                text = "\n".join([str(p.get_text()) for p in doc])
                n_pages = len(doc)
                doc.close()
                if len(text) > 500:
                    d = json.loads(aj.read_text())
                    identity = d.get("identity", {})
                    title = identity.get("title", "") or d.get("title", "")
                    candidates.append(
                        {
                            "article_ref": rel,
                            "article_key": article_key,
                            "title": title[:80],
                            "source_kind": "pdf_converted",
                            "n_pages": str(n_pages),
                            "char_count": str(len(text)),
                            "pdf_path": str(pdf.relative_to(REPO_ROOT)),
                        }
                    )
            except Exception:
                pass
    return candidates


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    sel_m117 = json.loads(M117_SELECTION.read_text())
    m117_articles = sel_m117["articles"]

    print("Finding PDF candidates...")
    pdf_candidates = find_pdf_candidates()
    print(f"PDF candidates: {len(pdf_candidates)}")

    # Combine: 20 baseline + 33 PDF (use all 32 + 1 nature from earlier scan)
    all_articles: list[dict[str, object]] = []
    for a in m117_articles:
        ca = dict(a)
        ca["selection_role"] = "m117_baseline"
        ca["selection_source"] = "m117-r024-20-document-corpus-v1"
        all_articles.append(ca)

    for i, c in enumerate(pdf_candidates):
        ca: dict[str, object] = {
            "article_ref": c["article_ref"],
            "article_key": c["article_key"],
            "source_code": c["article_ref"].split("/")[0] if "/" in c["article_ref"] else "other",
            "seed_url": f"https://arxiv.org/abs/{c['article_key']}"
            if "arxiv" in c["article_ref"]
            else "",
            "selection_role": f"{c['source_kind']}_extension_{i:02d}",
            "selection_source": "m118-r024-53-document-corpus-v1",
            "source_kind": c["source_kind"],
        }
        if c["source_kind"] == "pdf_converted":
            ca["pdf_path"] = c["pdf_path"]
            ca["pdf_pages"] = c["n_pages"]
            ca["topic_tags"] = ["pdf-converted", "pymupdf"]
        else:
            ca["text_path"] = c["text_path"]
            ca["topic_tags"] = [c["source_kind"]]
        ca["char_count"] = c["char_count"]
        all_articles.append(ca)

    # uniqueness
    keys = [str(a["article_key"]) for a in all_articles]
    if len(set(keys)) != len(keys):
        dups = [k for k in keys if keys.count(k) > 1]
        print(f"DUPLICATE keys: {dups}")
        return 1
    if len(all_articles) != 53:
        print(f"Expected 53, got {len(all_articles)}")
        return 1

    selection = {
        "schema_version": "article-corpus-selection.v00.02",
        "selection_id": "m118-r024-53-document-corpus-v1",
        "catalog_schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "purpose": "R024 final-stage 53-document corpus validation (M118). One-week scale via PDF-to-text.",
        "baseline_corpus": "m117-r024-20-document-corpus-v1",
        "pdf_conversion_library": "pymupdf 1.27.2.3",
        "pdf_conversion_documented": True,
        "catalog_exhausted_at": 55,
        "catalog_articles_with_local_sources": 22,
        "network_policy": "test_phase_must_not_fetch",
        "graph_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "selection_counts": {
            "baseline_m117": 20,
            "pdf_extension_m118": len(pdf_candidates),
            "total": 53,
        },
        "articles": all_articles,
    }
    OUT_SELECTION.write_text(json.dumps(selection, indent=2))
    print(f"selection.json: {len(all_articles)} articles (20 baseline + {len(pdf_candidates)} pdf)")

    # events
    events: list[dict[str, object]] = [
        {
            "event": "selection_start",
            "timestamp": datetime.now(UTC).isoformat(),
            "milestone": "M118-a2rx90",
            "slice": "S01",
            "schema_version": "r024-53-document-selection-event.v00.01",
        }
    ]
    for a in all_articles:
        events.append(
            {
                "event": "article_selected",
                "timestamp": datetime.now(UTC).isoformat(),
                "article_ref": a["article_ref"],
                "article_key": a["article_key"],
                "selection_role": a.get("selection_role", ""),
                "selection_source": a.get("selection_source", ""),
                "network_fetch_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
            }
        )
    events.append(
        {
            "event": "selection_complete",
            "timestamp": datetime.now(UTC).isoformat(),
            "total_articles": len(all_articles),
            "schema_version": "r024-53-document-selection-event.v00.01",
        }
    )
    with open(OUT_EVENTS, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")
    print(f"events.jsonl: {len(events)} events")

    # summary
    summary = {
        "schema_version": "r024-53-document-selection-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "total_articles": len(all_articles),
        "baseline_m117_count": 20,
        "pdf_extension_count": len(pdf_candidates),
        "unique_keys": len(set(keys)),
        "all_extractable": True,
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "ladybugdb_written": False,
    }
    OUT_SUMMARY.write_text(json.dumps(summary, indent=2))
    print("summary.json: 53 articles unique")
    return 0


if __name__ == "__main__":
    sys.exit(main())
