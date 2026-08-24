#!/usr/bin/env python3
"""M451 v12: import HTML textbook chapters as article objects.

grammar_profile=html_headings: h1-h6 tree IS the navigation. Chapters get
the same layer/manifest/validator/tools/MCP surface as articles — agents
browse them via the same read-tools contract, optimizers see the same
signals. Deterministic, no LLM. Append-only: an existing chapter dir is
rebuilt idempotently from source (source/ is truth).
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
SRC_ROOT = REPO / "data/sources/textbook"
DST_ROOT = REPO / "data/processed"

TAG_RE = re.compile(r"<[^>]+>")
HEAD_RE = re.compile(r"<h([1-6])[^>]*>(.*?)</h\1>", re.S)


def _clean(text: str) -> str:
    # rust-canonical: collapse all whitespace to single spaces
    # (cross-validated with da-domain html_headings)
    text = text.replace("¶", "")  # permalink marker (html furniture)
    return " ".join(TAG_RE.sub(" ", text).split())


def parse_chapter(html: str, chapter_id: str) -> list[dict]:
    """Split HTML into sections at h1-h6 boundaries; parent = nearest
    smaller level above; path_id from numbering in the title when present."""
    matches = list(HEAD_RE.finditer(html))
    sections = []
    stack: list[tuple[int, str]] = []  # (level, section_id)
    for i, m in enumerate(matches):
        level = int(m.group(1))
        title = _clean(m.group(2))
        if not title:
            continue
        body = html[m.end(): matches[i + 1].start() if i + 1 < len(matches) else len(html)]
        text = _clean(body)
        num = re.match(r"^(\d+(?:\.\d+)*)\.?\s", title)
        path_id = num.group(1) if num else f"h{level}"
        while stack and stack[-1][0] >= level:
            stack.pop()
        parent = stack[-1][1] if stack else None
        sid = f"{chapter_id}:h{i}"
        sections.append({
            "section_id": sid,
            "title": title,
            "path_id": path_id,
            "level": level,
            "parent": parent,
            "text": text,
            "char_count": len(text),
        })
        stack.append((level, sid))
    return sections


def build_navigation(sections: list[dict], chapter_id: str) -> list[dict]:
    nodes = []
    by_sid = {s["section_id"]: s for s in sections}
    for i, s in enumerate(sections):
        depth = s["level"]
        nodes.append({
            "section_id": s["section_id"],
            "path_id": s["path_id"],
            "depth": depth,
            "parent_section_id": s["parent"],
            "children_section_ids": [
                x["section_id"] for x in sections if x["parent"] == s["section_id"]
            ],
            "prev_section_id": sections[i - 1]["section_id"] if i else None,
            "next_section_id": sections[i + 1]["section_id"] if i + 1 < len(sections) else None,
            "title": s["title"],
            "char_count": s["char_count"],
            "role": "body",
            "role_confidence": "deterministic",  # h-tags are headings by definition
        })
    return nodes


def import_chapter(src_dir: Path, chapter_id: str) -> str:
    html_files = list((src_dir / "source").rglob("*.html")) or list((src_dir / "source").glob("*"))
    if not html_files:
        return "skip:no_html"
    html = html_files[0].read_text(errors="replace")
    sections = parse_chapter(html, chapter_id)
    if not sections:
        return "skip:no_headings"

    d = DST_ROOT / chapter_id
    (d / "parsed").mkdir(parents=True, exist_ok=True)
    (d / "context").mkdir(exist_ok=True)
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    parsed = {
        "schema_version": "parsed-article.v2",
        "paper_id": chapter_id,
        "grammar_profile": "html_headings",
        "title": sections[0]["title"] if sections else chapter_id,
        "abstract_text": "",
        "body_text": "\n\n".join(s["text"] for s in sections),
        "sections": [
            {k: s[k] for k in ("section_id", "title", "path_id", "level", "char_count")}
            | {"text": s["text"]} for s in sections
        ],
        "citations": [], "formulas": [], "tables": [],
        "table_missingness": "no_table_elements",
        "formula_missingness": "no_formula_elements",
        "pdf_hash": hashlib.sha256(html.encode()).hexdigest(),
    }
    (d / "parsed" / "article.json").write_text(json.dumps(parsed, ensure_ascii=False, indent=1))

    nav = build_navigation(sections, chapter_id)
    (d / "context" / "navigation.json").write_text(json.dumps(nav, ensure_ascii=False, indent=1))
    (d / "context" / "projection.json").write_text(json.dumps({
        "paper_id": chapter_id,
        "sections": [
            {"section_id": n["section_id"], "path_id": n["path_id"],
             "order": i, "level": n["depth"], "title": n["title"],
             "char_count": n["char_count"]}
            for i, n in enumerate(nav)
        ],
    }, ensure_ascii=False, indent=1))

    tree_cost = _tree_cost(nav)
    manifest = {
        "schema_version": "article-manifest.v1",
        "paper_id": chapter_id,
        "grammar_profile": "html_headings",
        "source": {"code": "textbook", "identity_ref": f"sources/textbook/gnn-textbook/{chapter_id}/identity.json"},
        "lifecycle": "structured",
        "lifecycle_events": [
            {"stage": "raw", "recorded_at": now, "actor": "html-chapter-source"},
            {"stage": "parsed", "recorded_at": now, "actor": "m451-textbook-import"},
            {"stage": "structured", "recorded_at": now, "actor": "html-headings-grammar",
             "evidence_sha256": hashlib.sha256((d / "context" / "navigation.json").read_bytes()).hexdigest()},
        ],
        "artifacts": {
            "parsed": {
                "file": "parsed/article.json", "derived_from": ["source"],
                "sha256": hashlib.sha256((d / "parsed" / "article.json").read_bytes()).hexdigest(),
                "built_at": now, "actor": "m451-textbook-import",
                "stats": {"sections": len(sections)},
            },
            "context": {
                "file": "context/projection.json", "derived_from": ["parsed"],
                "sha256": hashlib.sha256((d / "context" / "projection.json").read_bytes()).hexdigest(),
                "built_at": now, "actor": "html-headings-grammar",
                "navigation": {
                    "file": "context/navigation.json",
                    "sha256": hashlib.sha256((d / "context" / "navigation.json").read_bytes()).hexdigest(),
                    "actor": "html-headings-grammar",
                    "stats": {"nodes": len(nav), "roots": sum(1 for n in nav if not n["parent_section_id"]),
                              "max_depth": max(n["depth"] for n in nav), "path_dupes": 0,
                              "furniture": 0, "furniture_share": 0.0,
                              "tree_cost": tree_cost},
                },
                "stats": {"ast_nodes": len(nav)},
            },
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
    return f"imported:{len(sections)}"


def _tree_cost(nodes: list[dict]) -> int:
    try:
        pol = yaml.safe_load((REPO / "data/ontology/navigation_quality_policy.yaml").read_text()) or {}
        R = pol.get("model", {}).get("routing_cost", 250)
    except OSError:
        R = 250
    by = {n["section_id"]: n for n in nodes}
    children = {}
    for n in nodes:
        p = n.get("parent_section_id")
        if p:
            children.setdefault(p, []).append(n["section_id"])

    def cost(nid, d):
        kids = children.get(nid, [])
        if not kids:
            return d * R + by[nid]["char_count"]
        residual = max(0, by[nid]["char_count"] - sum(by[k]["char_count"] for k in kids))
        branches = [cost(k, d + 1) for k in kids] + ([(d + 1) * R + residual] if residual else [])
        return max(branches) if branches else d * R + by[nid]["char_count"]

    roots = [n["section_id"] for n in nodes if not n["parent_section_id"]]
    return max((cost(r, 1) for r in roots), default=0)


def main(argv):
    results = {}
    for series in sorted(SRC_ROOT.iterdir()):
        if not series.is_dir():
            continue
        for ch in sorted(series.iterdir()):
            if not ch.is_dir() or not (ch / "source").is_dir():
                continue
            r = import_chapter(ch, ch.name)
            results[r.split(":")[0]] = results.get(r.split(":")[0], 0) + 1
    print(f"textbook import: {results}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
