#!/usr/bin/env python3
"""M451: DOI-first article import (W-D rehearsal, one ACM paper).

Acquires a publisher-DOI paper through a legal mirror (SIGOPS hosts the
CC-BY HotOS PDF), registers it under source_code=acm, parses via GROBID
(structural profile per D204), and materializes the standard article
layers (parsed -> context/navigation -> manifest) with DOI identity.
Deterministic; no LLM. The Rust SourceAcquisitionPort (W-D) supersedes
this reference path later.
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
import time
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parent.parent
GROBID = "http://127.0.0.1:8070/api/processFulltextDocument"

DOI = "10.1145/3713082.3730397"
PAPER_ID = DOI.replace("/", "-")           # dir-safe id, identity stays the DOI
MIRROR = "https://sigops.org/s/conferences/hotos/2025/papers/hotos25-426.pdf"
SRC_DIR = REPO / f"data/sources/acm/{PAPER_ID}/source"
DST = REPO / "data/processed" / PAPER_ID

TEI_NS = {"t": "http://www.tei-c.org/ns/1.0"}


def acquire(pdf_path: Path) -> str:
    pdf_path.parent.mkdir(parents=True, exist_ok=True)
    if pdf_path.exists():
        return "cache"
    req = urllib.request.Request(MIRROR, headers={"User-Agent": "daily-archive/0.1"})
    with urllib.request.urlopen(req, timeout=120) as r:
        pdf_path.write_bytes(r.read())
    return "fetched"


def grobid_parse(pdf_path: Path) -> str:
    body = pdf_path.read_bytes()
    boundary = "----m451doi" + str(int(time.time()))
    parts = [
        f"--{boundary}\r\nContent-Disposition: form-data; name=\"consolidateCitations\"\r\n\r\n0\r\n".encode(),
        f"--{boundary}\r\nContent-Disposition: form-data; name=\"consolidateHeader\"\r\n\r\n0\r\n".encode(),
        f"--{boundary}\r\nContent-Disposition: form-data; name=\"input\"; filename=\"paper.pdf\"\r\n"
        "Content-Type: application/pdf\r\n\r\n".encode() + body + f"\r\n--{boundary}--\r\n".encode(),
    ]
    data = b"".join(parts)
    req = urllib.request.Request(
        GROBID, data=data,
        headers={"Content-Type": f"multipart/form-data; boundary={boundary}"})
    with urllib.request.urlopen(req, timeout=300) as r:
        return r.read().decode()


def parse_tei(tei: str) -> dict:
    root = ET.fromstring(re.sub(r"\sxmlns=", " xmlns=", tei))
    strip_ns = lambda tag: tag.split("}")[-1]

    def text_of(el) -> str:
        return "".join(el.itertext()).strip() if el is not None else ""

    title_el = root.find(".//t:fileDesc/t:titleStmt/t:title", TEI_NS)
    abstract_el = root.find(".//t:profileDesc/t:abstract", TEI_NS)

    sections, stack = [], []
    for div in root.findall(".//t:body//t:div", TEI_NS):
        head = div.find("t:head", TEI_NS)
        if head is None:
            continue
        # only nested-list flattening: GROBID divs nest; walk parents via stack of (n, sid)
        n = text_of(head)
        num = re.match(r"^(\d+(?:\.\d+)*)\.?\s", n or "")
        path_id = num.group(1) if num else f"s{len(sections) + 1}"
        while stack and int(stack[-1][0].count(".") or 0) >= int(path_id.count(".") or 0) and stack[-1][0] != path_id:
            stack.pop()
        sid = f"{PAPER_ID}:{len(sections)}"
        parent = stack[-1][1] if stack else None
        # section text: direct <p> of this div (not nested divs)
        paras = [text_of(p) for p in div.findall("t:p", TEI_NS)]
        text = "\n\n".join(p for p in paras if p)
        sections.append({
            "section_id": sid, "title": n, "path_id": path_id,
            "level": path_id.count(".") + 1 if num else 1,
            "parent_section_id": parent, "order_in_parent": 0,
            "text": text,
        })
        stack.append((path_id, sid))

    citations = []
    for i, bib in enumerate(root.findall(".//t:sourceDesc/t:listBibl/t:biblStruct", TEI_NS)):
        raw = " ".join(text_of(bib).split())
        title_el2 = bib.find(".//t:title", TEI_NS)
        doi_el = bib.find(".//t:idno[@type='DOI']", TEI_NS)
        arxiv_el = bib.find(".//t:idno[@type='arXiv']", TEI_NS)
        citations.append({
            "raw_text": raw,
            "doi": text_of(doi_el) or None,
            "arxiv_id": text_of(arxiv_el) or None,
            "title": text_of(title_el2) or None,
            "anchors": [],
        })

    return {
        "schema_version": "parsed-article.v2",
        "paper_id": PAPER_ID,
        "doi": DOI,
        "grammar_profile": "arxiv_pdf_dot",
        "title": text_of(title_el),
        "abstract_text": text_of(abstract_el),
        "body_text": "\n\n".join(s["text"] for s in sections),
        "sections": sections,
        "citations": citations,
        "formulas": [], "tables": [],
        "table_missingness": "no_table_elements",
        "formula_missingness": "no_formula_elements",
        "tei_xml": tei,
        "pdf_hash": hashlib.sha256((SRC_DIR / "paper.pdf").read_bytes()).hexdigest(),
    }


def main(argv):
    pdf = SRC_DIR / "paper.pdf"
    step = acquire(pdf)
    print(f"pdf: {step} ({pdf.stat().st_size} bytes)")

    # identity.json (source registration, nature-like pattern)
    ident = SRC_DIR.parent / "identity.json"
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    if not ident.exists():
        ident.write_text(json.dumps({
            "schema_version": "source-identity.v2",
            "article_key": PAPER_ID,
            "catalog_path": f"sources/acm/{PAPER_ID}",
            "source_code": "acm",
            "source_type": "publisher_mirror",
            "coarse_topic_code": "os-systems",
            "identity": {
                "title": "Understanding the limitations of pubsub systems",
                "doi": DOI,
                "canonical_url": f"https://doi.org/{DOI}",
                "publisher_url": f"https://dl.acm.org/doi/{DOI}",
                "mirror_url": MIRROR,
                "license": "CC-BY-4.0",
            },
            "publisher": "ACM",
            "topic_tags": ["pubsub", "distributed-systems"],
            "safety_flags": {"import_eligible": False},
            "registration_summary": {"registered_at": now, "by": "m451-doi-import"},
        }, ensure_ascii=False, indent=1))
        print("identity: written")

    tei = grobid_parse(pdf)
    parsed = parse_tei(tei)
    (DST / "parsed").mkdir(parents=True, exist_ok=True)
    (DST / "context").mkdir(exist_ok=True)
    (DST / "parsed" / "article.json").write_text(json.dumps(parsed, ensure_ascii=False, indent=1))
    print(f"parsed: {len(parsed['sections'])} sections, {len(parsed['citations'])} citations")

    # navigation (reuse the corpus builder)
    proj = {
        "paper_id": PAPER_ID,
        "sections": [
            {"section_id": s["section_id"], "path_id": s["path_id"], "order": i,
             "level": s["level"], "title": s["title"], "char_count": len(s["text"])}
            for i, s in enumerate(parsed["sections"])
        ],
    }
    (DST / "context" / "projection.json").write_text(json.dumps(proj, ensure_ascii=False, indent=1))
    # draft manifest first: the navigation builder merges into it
    mpath = DST / "manifest.yaml"
    if True:  # idempotent import: draft is always rebuilt from parsed truth
        mpath.write_text(yaml.safe_dump({
            "schema_version": "article-manifest.v1", "paper_id": PAPER_ID,
            "grammar_profile": "arxiv_pdf_dot",
            "source": {"code": "acm", "doi": DOI,
                       "identity_ref": f"sources/acm/{PAPER_ID}/identity.json"},
            "lifecycle": "parsed", "lifecycle_events": [],
            "artifacts": {
                "extracted": {
                    "present": False, "missingness": "extraction_not_run",
                    "derived_from": ["parsed"],
                },
                "context": {
                    "file": "context/projection.json", "derived_from": ["parsed"],
                    "sha256": "draft", "built_at": now, "actor": "m451-doi-import",
                    "stats": {"ast_nodes": len(parsed["sections"])},
                }
            },
        }, sort_keys=False, allow_unicode=True))
    sys.path.insert(0, str(REPO / "scripts"))
    import importlib.util
    spec = importlib.util.spec_from_file_location("navb", REPO / "scripts/m451_navigation_build.py")
    navb = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(navb)
    navb.build_navigation(PAPER_ID, REPO / "data/processed")

    manifest = {
        "schema_version": "article-manifest.v1",
        "paper_id": PAPER_ID,
        "grammar_profile": "arxiv_pdf_dot",
        "source": {"code": "acm", "doi": DOI,
                   "identity_ref": f"sources/acm/{PAPER_ID}/identity.json"},
        "lifecycle": "parsed",
        "lifecycle_events": [
            {"stage": "raw", "recorded_at": now, "actor": "doi-mirror-fetch",
             "evidence_sha256": parsed["pdf_hash"]},
            {"stage": "parsed", "recorded_at": now, "actor": "grobid-structural",
             "evidence_sha256": hashlib.sha256((DST / "parsed/article.json").read_bytes()).hexdigest()},
        ],
        "artifacts": {
            "parsed": {
                "file": "parsed/article.json", "derived_from": ["source"],
                "sha256": hashlib.sha256((DST / "parsed/article.json").read_bytes()).hexdigest(),
                "built_at": now, "actor": "grobid-structural",
                "stats": {"sections": len(parsed["sections"]),
                          "citations": len(parsed["citations"])},
            },
        },
    }
    # context entry by navigation builder already added; merge manually
    mpath = DST / "manifest.yaml"
    if mpath.exists():
        existing = yaml.safe_load(mpath.read_text())
        existing.update({k: v for k, v in manifest.items() if k != "artifacts"})
        existing["artifacts"].update(manifest["artifacts"])
        existing["artifacts"].setdefault("context", {
            "file": "context/projection.json", "derived_from": ["parsed"],
            "sha256": hashlib.sha256((DST / "context/projection.json").read_bytes()).hexdigest(),
            "built_at": now, "actor": "m451-navigation-builder",
            "stats": {"ast_nodes": len(parsed["sections"])},
        })
        manifest = existing
    mpath.write_text(yaml.safe_dump(manifest, sort_keys=False, allow_unicode=True))
    print(f"manifest: lifecycle={manifest['lifecycle']}, layers={list(manifest['artifacts'])}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
