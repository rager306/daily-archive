#!/usr/bin/env python3
"""M451 summaries layer build (reference): abstract.json per article.

Model + prompt come from DATA (models.yaml binding
paper-summary-generate-default + summary_eval_policy.yaml prompt).
ADR-055 lineage: cache key binds pid+model+prompt_sha256; the stored
record carries model/endpoint/fetched_at. Append-only: already
summarized articles skip (replay-safe). Honest pending on failures.
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

_SCRUB = None


def _scrub_injection(text: str):
    """Prompt-injection hardening (data/ontology/prompt_injection_policy.yaml).
    Returns (clean_text, n_replaced). DATA-driven patterns, never logged raw."""
    global _SCRUB
    if _SCRUB is None:
        try:
            cfg = yaml.safe_load((REPO / "data/ontology/prompt_injection_policy.yaml").read_text()) or {}
            _SCRUB = [(re.compile(p), cfg.get("replacement", "[scrubbed-injection]"))
                      for p in cfg.get("scrub_patterns", [])]
        except OSError:
            _SCRUB = []
    n = 0
    for pat, repl in _SCRUB:
        text, k = pat.subn(repl, text)
        n += k
    return text, n



REPO = Path(__file__).resolve().parent.parent
CACHE = REPO / "artifacts/pipeline_cache/summaries_v1"
POLICY = yaml.safe_load((REPO / "data/ontology/summary_eval_policy.yaml").read_text())
MODELS = yaml.safe_load((REPO / "models.yaml").read_text())


def default_summary_model() -> tuple[str, str]:
    """(endpoint, model_name) from the pinned binding (DATA)."""
    binding = next(b for b in MODELS["bindings"]
                   if b["binding_id"] == "paper-summary-generate-default")
    model_id = binding["model_id"]
    entry = next(m for m in MODELS["models"] if m["id"] == model_id)
    return entry["endpoint"], entry["model_name"]


ENDPOINT, MODEL = default_summary_model()
PROMPT = POLICY["prompt"]
PROMPT_SHA = hashlib.sha256(PROMPT.encode()).hexdigest()


def _router_key() -> str:
    import os
    key = os.environ.get("NINE_ROUTER_API_KEY", "")
    if not key:
        for ln in (REPO / ".env").read_text().splitlines():
            if ln.startswith("NINE_ROUTER_API_KEY="):
                key = ln.split("=", 1)[1].strip()
                break
    return key


def extract_json(raw: str):
    raw = re.sub(r"<think>.*?</think>", "", raw, flags=re.S)
    raw = re.sub(r"^```(?:json)?\s*|\s*```$", "", raw.strip())
    try:
        return json.loads(raw)
    except ValueError:
        m = re.search(r"\{.*\}", raw, re.S)
        if m:
            try:
                return json.loads(m.group(0))
            except ValueError:
                return None
    return None


def generate(pid: str, user: str, _scrubbed: int = 0) -> dict:
    """Cached LLM call; cache key = pid + model + prompt_sha256."""
    user, _scrubbed = _scrub_injection(user)
    CACHE.mkdir(parents=True, exist_ok=True)
    cf = CACHE / f"{pid}.json"
    if cf.exists():
        rec = json.loads(cf.read_text())
        if rec.get("ok"):
            return rec
    key = _router_key()
    body = json.dumps({
        "model": MODEL,
        "messages": [{"role": "system", "content": PROMPT},
                     {"role": "user", "content": user}],
        "temperature": 0.2,
        "max_tokens": 4000,
        "stream": False,
    }).encode()
    rec = {"pid": pid, "model": MODEL, "prompt_sha256": PROMPT_SHA,
           "endpoint": ENDPOINT, "ok": False, "injection_scrubbed": _scrubbed}
    for attempt in range(3):
        req = urllib.request.Request(ENDPOINT, data=body, headers={
            "Content-Type": "application/json",
            **({"Authorization": f"Bearer {key}"} if key else {})})
        t0 = time.time()
        try:
            with urllib.request.urlopen(req, timeout=240) as r:
                resp = json.loads(r.read().decode())
            content = (resp.get("choices") or [{}])[0].get("message", {}).get("content") or ""
            obj = extract_json(content)
            rec.update({"ok": obj is not None, "parsed": obj,
                        "latency_s": round(time.time() - t0, 1),
                        "fetched_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                        "usage": resp.get("usage", {})})
            break
        except urllib.error.HTTPError as e:
            rec["error"] = f"HTTP {e.code}"
            if e.code in (429, 500, 502, 503):
                time.sleep(15 * (attempt + 1))
                continue
            break
        except Exception as e:
            rec["error"] = str(e)[:120]
            break
    if rec["ok"]:
        cf.write_text(json.dumps(rec, ensure_ascii=False))
    return rec


def build_user(policy: dict, parsed: dict) -> str:
    maxc = policy["input"]["max_body_chars"]
    secs = parsed.get("sections") or []
    body = "\n\n".join(f"## {s.get('title','')}\n{s.get('text','')}" for s in secs)
    text = f"TITLE: {parsed.get('title','')}\n\nABSTRACT: {parsed.get('abstract_text','')}\n\n{body}"
    return text[:maxc] + "\n...[truncated]" if len(text) > maxc else text


def summarize(pid: str, root: Path) -> str:
    article = root / pid
    mp = article / "manifest.yaml"
    m = yaml.safe_load(mp.read_text())
    if "summarized" in [e["stage"] for e in m.get("lifecycle_events", [])]:
        return "skip"
    parsed = json.loads((article / "parsed" / "article.json").read_text())
    user = build_user(POLICY, parsed)
    rec = generate(pid, user)
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    art = m["artifacts"]

    if not rec.get("ok"):
        art["summaries"] = {"present": False,
                            "missingness": "llm_generation_failed",
                            "derived_from": ["parsed", "enriched"]}
        mp.write_text(yaml.safe_dump(m, sort_keys=False, allow_unicode=True))
        return "pending"

    obj = rec["parsed"]
    sdir = article / "summaries"
    sdir.mkdir(exist_ok=True)
    node = {
        "schema_version": "summary-node.v1",
        "paper_id": pid,
        "tldr": obj.get("tldr"),
        "contributions": obj.get("contributions"),
        "methods": obj.get("methods"),
        "results": obj.get("results"),
        "limitations": obj.get("limitations"),
        "provenance": {"model": MODEL, "prompt_sha256": PROMPT_SHA,
                       "fetched_at": rec["fetched_at"],
                       "binding": "paper-summary-generate-default"},
    }
    (sdir / "abstract.json").write_text(json.dumps(node, ensure_ascii=False, indent=1))
    art["summaries"] = {
        "file": "summaries/abstract.json",
        "derived_from": ["parsed", "enriched"],
        "sha256": hashlib.sha256((sdir / "abstract.json").read_bytes()).hexdigest(),
        "built_at": now,
        "actor": MODEL,
        "stats": {"tldr_chars": len(str(node["tldr"] or "")),
                  "contributions": len(obj.get("contributions") or [])},
    }
    m["lifecycle_events"].append({
        "stage": "summarized", "recorded_at": now, "actor": MODEL,
        "evidence_sha256": art["summaries"]["sha256"],
    })
    m["lifecycle"] = "summarized"
    mp.write_text(yaml.safe_dump(m, sort_keys=False, allow_unicode=True))
    return "summarized"


def main(argv):
    root = REPO / "data/processed"
    ids = argv[1:] or sorted(d.name for d in root.iterdir()
                             if d.is_dir() and (d / "manifest.yaml").exists())
    from concurrent.futures import ThreadPoolExecutor

    def work(pid):
        try:
            return summarize(pid, root)
        except Exception as e:
            return f"error:{str(e)[:80]}"

    with ThreadPoolExecutor(max_workers=8) as ex:
        results = list(ex.map(work, ids))
    counts = {}
    for r in results:
        counts[r] = counts.get(r, 0) + 1
    print(f"summaries: {counts} | model={MODEL}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
