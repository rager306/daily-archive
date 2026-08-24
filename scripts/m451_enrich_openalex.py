#!/usr/bin/env python3
"""M451 enriched stage (reference): OpenAlex work anchor per article.

ID-first (user principle): external identity = OpenAlex A-ID + ORCID.
TEI<->OpenAlex author binding is POSITIONAL with a surname check —
name-only matches are marked. Not-found works stay honest pending
(existing openalex_pending pattern), never fabricated.

Cache: artifacts/pipeline_cache/openalex_works/<paper_id>.json
(ADR-055 lineage: endpoint + fetched_at + http status in provenance).
Append-only: an already-enriched article is skipped (replay-safe).
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
import time
import urllib.request
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parent.parent
CACHE = REPO / "artifacts/pipeline_cache/openalex_works"
UA = {"User-Agent": "daily-archive/0.1 (mailto:research@example.org)"}


def _api_key() -> str:
    """OPENALEX_API_KEY from env or .env (never logged, never in YAML)."""
    key = "".__class__("OPENALEX_API_KEY")
    import os
    key = os.environ.get("OPENALEX_API_KEY", "")
    if not key:
        envf = REPO / ".env"
        if envf.exists():
            for ln in envf.read_text().splitlines():
                if ln.startswith("OPENALEX_API_KEY="):
                    key = ln.split("=", 1)[1].strip().strip('"').strip("'")
                    break
    return key


def _endpoint(pid: str) -> str:
    url = f"https://api.openalex.org/works/doi:10.48550/arXiv.{pid}?mailto=research@example.org"
    key = _api_key()
    if key:
        url += f"&api_key={key}"
    return url


def _norm(t):
    import unicodedata
    t = unicodedata.normalize("NFKD", t or "").lower()
    return re.sub(r"[^a-z0-9]+", "", t)


def fetch_work(pid: str) -> dict:
    """Cached OpenAlex work record or honest failure marker."""
    CACHE.mkdir(parents=True, exist_ok=True)
    cf = CACHE / f"{pid}.json"
    if cf.exists():
        try:
            rec = json.loads(cf.read_text())
            if rec.get("http_status") == 200 or rec.get("terminal"):
                return rec
        except ValueError:
            pass
    url = _endpoint(pid)
    try:
        req = urllib.request.Request(url, headers=UA)
        with urllib.request.urlopen(req, timeout=30) as r:
            body = json.loads(r.read().decode())
            rec = {"http_status": 200, "fetched_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "work": _slim(body)}
    except urllib.error.HTTPError as e:
        rec = {"http_status": e.code, "fetched_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "terminal": e.code in (404, 422), "work": None}
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        rec = {"http_status": 0, "fetched_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "terminal": False, "error": str(e)[:120], "work": None}
    cf.write_text(json.dumps(rec, ensure_ascii=False))
    return rec


def _slim(w: dict) -> dict:
    src = ((w.get("primary_location") or {}).get("source") or {})
    return {
        "work_id": w.get("id"),
        "title": w.get("title"),
        "publication_year": w.get("publication_year"),
        "cited_by_count": w.get("cited_by_count"),
        "source": {"id": src.get("id"), "display_name": src.get("display_name")} if src.get("id") else None,
        "authorships": [
            {
                "a_id": a["author"]["id"].rsplit("/", 1)[-1] if a["author"].get("id") else None,
                "display_name": a["author"].get("display_name"),
                "orcid": (a["author"].get("orcid") or "").rsplit("/", 1)[-1] or None,
                "institutions": [i.get("display_name") for i in (a.get("institutions") or [])][:2],
            }
            for a in (w.get("authorships") or [])
        ],
        "topics": [
            {"id": t.get("id"), "display_name": t.get("display_name"), "score": t.get("score")}
            for t in (w.get("topics") or [])[:5]
        ],
    }


def tei_authors(parsed: dict) -> list:
    """Full names + surnames from TEI header, in order."""
    m = re.search(r"<fileDesc>(.*?)</fileDesc>", parsed.get("tei_xml") or "", re.S)
    out = []
    if m:
        for pn in re.findall(r"<persName>(.*?)</persName>", m.group(1), re.S):
            fores = re.findall(r"<forename[^>]*>([^<]+)</forename>", pn)
            surs = re.findall(r"<surname[^>]*>([^<]+)</surname>", pn)
            if surs:
                out.append({"full": " ".join(fores + surs), "surname": surs[-1],
                            "author_key": _norm("".join(f[0] for f in fores) + "".join(surs))})
    return out


def enrich(pid: str, root: Path) -> str:
    article = root / pid
    manifest_path = article / "manifest.yaml"
    manifest = yaml.safe_load(manifest_path.read_text())
    stages = [e["stage"] for e in manifest.get("lifecycle_events", [])]
    if "enriched" in stages:
        return "skip"

    parsed = json.loads((article / "parsed" / "article.json").read_text())
    rec = fetch_work(pid)
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    art = manifest["artifacts"]

    if rec["http_status"] != 200 or not rec["work"]:
        # honest pending (audit pattern): no fabrication
        art["enriched"] = {
            "not_reached": False,
            "present": False,
            "missingness": "openalex_not_found" if rec.get("terminal") else "openalex_fetch_failed",
            "derived_from": ["parsed"],
        }
        manifest_path.write_text(yaml.safe_dump(manifest, sort_keys=False, allow_unicode=True))
        return "pending"

    w = rec["work"]
    # authors: POSITIONAL binding to TEI, surname check marks confidence
    tei_aus = tei_authors(parsed)
    authors = []
    for i, sh in enumerate(w["authorships"]):
        entry = {
            "position": i,
            "openalex_a_id": sh["a_id"],
            "display_name": sh["display_name"],
            "orcid": sh["orcid"],
            "institutions": sh["institutions"],
        }
        if i < len(tei_aus):
            entry["author_key"] = tei_aus[i]["author_key"]
            entry["tei_full_name"] = tei_aus[i]["full"]
            surname_ok = _norm(tei_aus[i]["surname"]) in _norm(sh["display_name"])
            entry["binding"] = "position_surname_confirmed" if surname_ok else "position_surname_mismatch"
        else:
            entry["binding"] = "openalex_only"
        authors.append(entry)
    # rust-canonical: TEI authors beyond OpenAlex rows are honest tei_only
    for j, t in enumerate(tei_aus[len(w["authorships"]):],
                          start=len(w["authorships"])):
        authors.append({
            "position": j,
            "openalex_a_id": None, "display_name": None, "orcid": None,
            "institutions": [],
            "author_key": t["author_key"], "tei_full_name": t["full"],
            "binding": "tei_only",
        })
    (article / "enriched").mkdir(exist_ok=True)
    (article / "enriched" / "authors.json").write_text(json.dumps(authors, ensure_ascii=False, indent=1))

    (article / "enriched" / "venue.json").write_text(json.dumps(w["source"] or {}, ensure_ascii=False, indent=1))
    (article / "enriched" / "topics.json").write_text(json.dumps(w["topics"], ensure_ascii=False, indent=1))

    prov = {
        "work_id": w["work_id"],
        "endpoint": "api.openalex.org/works/doi:10.48550/arXiv.{pid}",
        "fetched_at": rec["fetched_at"],
        "http_status": rec["http_status"],
        "cache_key": f"openalex_work:{pid}",
    }
    (article / "enriched" / "provenance.json").write_text(json.dumps(prov, ensure_ascii=False, indent=1))

    art["enriched"] = {
        "file": "enriched/authors.json",
        "files": ["enriched/authors.json", "enriched/venue.json", "enriched/topics.json", "enriched/provenance.json"],
        "derived_from": ["parsed"],
        "sha256": hashlib.sha256((article / "enriched" / "provenance.json").read_bytes()).hexdigest(),
        "built_at": now,
        "actor": "openalex-enricher",
        "stats": {
            "authors": len(authors),
            "with_orcid": sum(1 for a in authors if a.get("orcid")),
            "with_a_id": sum(1 for a in authors if a.get("openalex_a_id")),
            "surname_confirmed": sum(1 for a in authors if a.get("binding") == "position_surname_confirmed"),
            "binding_mismatch": sum(1 for a in authors if a.get("binding") == "position_surname_mismatch"),
            "topics": len(w["topics"]),
            "cited_by_count": w.get("cited_by_count"),
        },
        "inputs": ["api.openalex.org (polite pool)"],
    }

    # lifecycle event: append-only, state derived downstream
    manifest["lifecycle_events"].append({
        "stage": "enriched", "recorded_at": now,
        "actor": "openalex-enricher",
        "evidence_sha256": art["enriched"]["sha256"],
    })
    manifest["lifecycle"] = manifest["lifecycle_events"][-1]["stage"]
    manifest_path.write_text(yaml.safe_dump(manifest, sort_keys=False, allow_unicode=True))
    return "enriched"


def main(argv):
    root = REPO / "data/processed"
    ids = argv[1:] or sorted(d.name for d in root.iterdir()
                             if d.is_dir() and (d / "manifest.yaml").exists())
    from concurrent.futures import ThreadPoolExecutor

    def work(pid):
        try:
            return enrich(pid, root)
        except Exception as e:  # per-article isolation: never kill the batch
            return f"error:{str(e)[:80]}"

    # network latency ~6s per request from this host; OpenAlex allows
    # 10 req/s -> 8 workers stay well inside the limit (~1.4 req/s)
    with ThreadPoolExecutor(max_workers=8) as ex:
        results = list(ex.map(work, ids))
    counts = {}
    for r in results:
        counts[r] = counts.get(r, 0) + 1
    print(f"enrich: {counts}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
