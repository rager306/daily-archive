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
import threading
import time
import urllib.request
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parent.parent
CACHE = REPO / "artifacts/pipeline_cache/openalex_works"
UA = {"User-Agent": "daily-archive/0.1 (mailto:research@example.org)"}

# ── ADR-073: citracer resilience trio ──────────────────────────────────────
BREAKER_THRESHOLD = 5       # подряд сбоев до отключения upstream
BREAKER_COOLDOWN_S = 120.0  # сколько upstream отключён
BATCH_SIZE = 50             # DOI за один вызов (лимит OpenAlex filter)


class CircuitBreaker:
    """Пер-upstream выключатель (citracer _orev_circuit_*): после N подряд
    сбоев upstream пропускается cooldown секунд; успех сбрасывает счётчик.
    Потокобезопасен — fetcher работает 8 воркерами."""

    def __init__(self, threshold: int = BREAKER_THRESHOLD,
                 cooldown: float = BREAKER_COOLDOWN_S) -> None:
        self._threshold = threshold
        self._cooldown = cooldown
        self._lock = threading.Lock()
        self._failures = 0
        self._tripped_at: float | None = None

    def allow(self) -> bool:
        with self._lock:
            if self._tripped_at is None:
                return True
            if time.time() - self._tripped_at > self._cooldown:
                self._tripped_at = None
                self._failures = 0
                return True
            return False

    def record_failure(self) -> None:
        with self._lock:
            self._failures += 1
            if self._failures >= self._threshold and self._tripped_at is None:
                self._tripped_at = time.time()

    def record_success(self) -> None:
        with self._lock:
            self._failures = 0


def purge_negative_cache() -> int:
    """Self-heal негативного кэша (citracer purge_negatives): терминальные
    404/422 записи устаревают, когда (a) upstream флапал в момент запроса,
    (b) наша логика резолва починена (citation-salvage), (c) arXiv-статьи
    индексируются OpenAlex с задержкой в дни/недели. Позитивы не трогаем."""
    purged = 0
    for cf in CACHE.glob("*.json"):
        try:
            rec = json.loads(cf.read_text())
        except ValueError:
            cf.unlink(missing_ok=True)
            purged += 1
            continue
        if rec.get("http_status") in (404, 422) and rec.get("terminal"):
            cf.unlink(missing_ok=True)
            purged += 1
    return purged


def _batch_endpoint(dois: list[str]) -> str:
    from urllib.parse import quote
    base = "https://api.openalex.org/works?filter=" + quote("|".join(f"doi:{d}" for d in dois), safe="")
    key = _api_key()
    sep = "&" if "?" in base else "?"
    return f"{base}{sep}mailto=research@example.org" + (f"&api_key={key}" if key else "")


def fetch_batch(pids: list[str], breaker: CircuitBreaker) -> dict[str, dict]:
    """Батч-загрузка (citracer batch_enrich): один вызов на BATCH_SIZE DOI.
    Возвращает {pid: cache_record}; статьи, не вернувшиеся в ответе,
    получают честную терминальную 404-запись."""
    arxiv_doi = {pid: f"10.48550/arXiv.{pid}" for pid in pids}
    out: dict[str, dict] = {}
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    for i in range(0, len(pids), BATCH_SIZE):
        chunk = pids[i:i + BATCH_SIZE]
        if not breaker.allow():
            for pid in chunk:
                out[pid] = {"http_status": 0, "fetched_at": now, "terminal": False,
                            "error": "circuit_open", "work": None}
            continue
        try:
            req = urllib.request.Request(_batch_endpoint([arxiv_doi[p] for p in chunk]), headers=UA)
            with urllib.request.urlopen(req, timeout=60) as r:
                body = json.loads(r.read().decode())
            breaker.record_success()
            by_doi = {}
            for w in body.get("results", []):
                doi = (w.get("doi") or "").replace("https://doi.org/", "").lower()
                by_doi[doi] = w
            for pid in chunk:
                w = by_doi.get(arxiv_doi[pid].lower())
                if w:
                    out[pid] = {"http_status": 200, "fetched_at": now, "work": _slim(w)}
                else:
                    out[pid] = {"http_status": 404, "fetched_at": now, "terminal": True, "work": None}
        except urllib.error.HTTPError as e:
            breaker.record_failure()
            for pid in chunk:
                out[pid] = {"http_status": e.code, "fetched_at": now,
                            "terminal": e.code in (404, 422), "work": None}
        except (urllib.error.URLError, TimeoutError, OSError, ValueError) as e:
            breaker.record_failure()
            for pid in chunk:
                out[pid] = {"http_status": 0, "fetched_at": now, "terminal": False,
                            "error": str(e)[:120], "work": None}
    # материализуем в индивидуальные кэш-файлы
    CACHE.mkdir(parents=True, exist_ok=True)
    for pid, rec in out.items():
        (CACHE / f"{pid}.json").write_text(json.dumps(rec, ensure_ascii=False))
    return out


def needs_fetch(pid: str) -> bool:
    """Кэш отсутствует или нетерминальный сбой → нужен запрос."""
    cf = CACHE / f"{pid}.json"
    if not cf.exists():
        return True
    try:
        rec = json.loads(cf.read_text())
    except ValueError:
        return True
    return rec.get("http_status") != 200 and not rec.get("terminal")


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
    no_refetch = "--no-refetch" in argv
    argv = [a for a in argv if a != "--no-refetch"]
    root = REPO / "data/processed"
    ids = argv[1:] or sorted(d.name for d in root.iterdir()
                             if d.is_dir() and (d / "manifest.yaml").exists())

    # ADR-073 citracer trio, шаг 1: самолечение негативного кэша
    if not no_refetch:
        purged = purge_negative_cache()
        if purged:
            print(f"purged {purged} stale negative cache entries")

    # шаг 2: батч-загрузка отсутствующих/негативных (50 DOI за вызов)
    breaker = CircuitBreaker()
    todo = [pid for pid in ids if needs_fetch(pid)]
    if todo:
        fetched = fetch_batch(todo, breaker)
        ok = sum(1 for r in fetched.values() if r.get("http_status") == 200)
        print(f"batch: {ok}/{len(todo)} works fetched ({(len(todo) + BATCH_SIZE - 1) // BATCH_SIZE} requests)")

    # шаг 3: существующий enrich по кэшу (манифесты), 8 воркеров
    from concurrent.futures import ThreadPoolExecutor

    def work(pid):
        try:
            return enrich(pid, root)
        except Exception as e:  # per-article isolation: never kill the batch
            return f"error:{str(e)[:80]}"

    with ThreadPoolExecutor(max_workers=8) as ex:
        results = list(ex.map(work, ids))
    counts = {}
    for r in results:
        counts[r] = counts.get(r, 0) + 1
    print(f"enrich: {counts}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
