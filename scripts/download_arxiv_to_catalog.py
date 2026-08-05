#!/usr/bin/env python3
"""Download arXiv PDFs into the canonical article catalog (AGENTS rule 1 / M061 pattern).

Layout:
  data/article_catalog/article_catalog/arxiv/<category>/<id>/source/<id>.pdf

- Category from arXiv API when available; fallback `unknown`
- SHA256 / existing-file idempotency
- Retries + backoff on HTTP 429/503
- Prefer `curl` for PDF fetch when available (better redirect/rate handling)

Usage:
  python3 scripts/download_arxiv_to_catalog.py --ids-file artifacts/gepa_hub_research/missing_arxiv.txt
  python3 scripts/download_arxiv_to_catalog.py 2507.19457 2212.14024
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "data" / "article_catalog" / "article_catalog" / "arxiv"
API = "http://export.arxiv.org/api/query?id_list={ids}"
ATOM = "{http://www.w3.org/2005/Atom}"
ARXIV_NS = "{http://arxiv.org/schemas/atom}"
UA = "daily-archive/gepa-hub-research (local research bot)"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def http_get(url: str, timeout: int = 90) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "*/*"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return resp.read()


def fetch_meta_one(aid: str, retries: int = 6) -> dict:
    """Fetch primary_category + title for one id with backoff."""
    url = API.format(ids=aid)
    last = "unknown"
    for attempt in range(retries):
        try:
            xml = http_get(url, timeout=60)
            root = ET.fromstring(xml)
            entry = root.find(f"{ATOM}entry")
            if entry is None:
                return {"category": "unknown", "title": ""}
            title_el = entry.find(f"{ATOM}title")
            title = re.sub(r"\s+", " ", (title_el.text or "").strip()) if title_el is not None else ""
            primary = entry.find(f"{ARXIV_NS}primary_category")
            cat = "unknown"
            if primary is not None:
                cat = (primary.get("term") or "unknown").replace(".", "-")
            return {"category": cat, "title": title}
        except urllib.error.HTTPError as e:
            last = f"HTTP {e.code}"
            if e.code in (429, 503):
                time.sleep(8 * (attempt + 1))
                continue
            break
        except Exception as e:
            last = f"{type(e).__name__}: {e}"
            time.sleep(3 * (attempt + 1))
    print(f"  meta fallback for {aid}: {last}")
    return {"category": "unknown", "title": ""}


def curl_download(url: str, dest: Path) -> tuple[bool, str]:
    if not shutil.which("curl"):
        return False, "curl not found"
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(".pdf.part")
    cmd = [
        "curl",
        "-fsSL",
        "--retry",
        "5",
        "--retry-delay",
        "5",
        "--retry-all-errors",
        "-A",
        UA,
        "-o",
        str(tmp),
        url,
    ]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
    except Exception as e:
        return False, f"curl exception: {e}"
    if r.returncode != 0:
        tmp.unlink(missing_ok=True)
        return False, f"curl exit {r.returncode}: {(r.stderr or '')[:120]}"
    data = tmp.read_bytes()
    if len(data) < 1000 or not data[:5].startswith(b"%PDF"):
        tmp.unlink(missing_ok=True)
        return False, f"not a PDF (len={len(data)})"
    digest = hashlib.sha256(data).hexdigest()
    tmp.replace(dest)
    return True, f"wrote {dest} sha256={digest[:12]} bytes={len(data)}"


def download_pdf(aid: str, dest: Path) -> tuple[bool, str]:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists() and dest.stat().st_size > 1000:
        try:
            with dest.open("rb") as f:
                if f.read(5).startswith(b"%PDF"):
                    return True, f"idempotent skip existing bytes={dest.stat().st_size}"
        except OSError:
            pass
    urls = [
        f"https://export.arxiv.org/pdf/{aid}",
        f"https://arxiv.org/pdf/{aid}.pdf",
        f"https://arxiv.org/pdf/{aid}",
        f"http://export.arxiv.org/pdf/{aid}",
    ]
    last = "unknown"
    for url in urls:
        ok, msg = curl_download(url, dest)
        if ok:
            return True, msg
        last = msg
        time.sleep(2)
    return False, last


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("ids", nargs="*", help="arXiv ids")
    ap.add_argument("--ids-file", type=Path)
    ap.add_argument("--catalog", type=Path, default=CATALOG)
    ap.add_argument("--sleep", type=float, default=4.0, help="seconds between items")
    ap.add_argument("--skip-meta", action="store_true", help="use category=unknown (no API)")
    args = ap.parse_args()
    ids: list[str] = list(args.ids)
    if args.ids_file:
        for line in args.ids_file.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                ids.append(line)
    seen: set[str] = set()
    aids: list[str] = []
    for a in ids:
        a = a.strip()
        if a and a not in seen:
            seen.add(a)
            aids.append(a)
    if not aids:
        print("no ids", file=sys.stderr)
        return 2

    print(f"download {len(aids)} ids into {args.catalog} (sleep={args.sleep}s)")
    ok = skip = fail = 0
    results = []
    for i, aid in enumerate(aids, 1):
        print(f"[{i}/{len(aids)}] {aid}")
        if args.skip_meta:
            m = {"category": "unknown", "title": ""}
        else:
            m = fetch_meta_one(aid)
            time.sleep(1.0)
        cat = m["category"]
        dest = args.catalog / cat / aid / "source" / f"{aid}.pdf"
        success, msg = download_pdf(aid, dest)
        if success and "idempotent" in msg:
            status = "skip"
            skip += 1
        elif success:
            status = "ok"
            ok += 1
        else:
            status = "fail"
            fail += 1
        print(f"  [{status}] cat={cat} {msg}")
        results.append(
            {
                "id": aid,
                "category": cat,
                "title": m.get("title", ""),
                "status": status,
                "msg": msg,
                "path": str(dest),
            }
        )
        time.sleep(args.sleep)

    summary = {"ok": ok, "skip": skip, "fail": fail, "total": len(aids), "results": results}
    out = ROOT / "artifacts" / "gepa_hub_research" / "download_results.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(summary, indent=2) + "\n")
    print(f"summary ok={ok} skip={skip} fail={fail} → {out}")
    return 0 if fail == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
