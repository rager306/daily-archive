#!/usr/bin/env python3
"""M451 background runner (reference): lazy resource-heavy tasks.

Registry-driven (data/ontology/background_tasks.yaml): the loop collects
lazy-request markers, applies cost-profile guards (rate/budget/enabled),
executes with cache fingerprints, updates statuses in manifests.
Append-only; failures are honest (failed), never silent.

Current executor: per_section_summaries (llm profile, agnes via the
pinned binding, prompt from DATA file). debug_html_rebuild executor
arrives with the debug layer.
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
REGISTRY = yaml.safe_load((REPO / "data/ontology/background_tasks.yaml").read_text())
MODELS = yaml.safe_load((REPO / "models.yaml").read_text())
CACHE = REPO / "artifacts/pipeline_cache/bg_per_section"
STATE_FILE = REPO / "artifacts/pipeline_cache/bg_state.json"


def _router_key() -> str:
    import os
    key = os.environ.get("NINE_ROUTER_API_KEY", "")
    if not key:
        for ln in (REPO / ".env").read_text().splitlines():
            if ln.startswith("NINE_ROUTER_API_KEY="):
                key = ln.split("=", 1)[1].strip()
                break
    return key


def binding_model(binding_id: str) -> tuple[str, str]:
    b = next(x for x in MODELS["bindings"] if x["binding_id"] == binding_id)
    m = next(x for x in MODELS["models"] if x["id"] == b["model_id"])
    return m["endpoint"], m["model_name"]


def extract_json_array(raw: str):
    raw = re.sub(r"<think>.*?</think>", "", raw, flags=re.S)
    raw = re.sub(r"^```(?:json)?\s*|\s*```$", "", raw.strip())
    try:
        v = json.loads(raw)
        return v if isinstance(v, list) else None
    except ValueError:
        m = re.search(r"\[.*\]", raw, re.S)
        if m:
            try:
                v = json.loads(m.group(0))
                return v if isinstance(v, list) else None
            except ValueError:
                return None
    return None


# ── rate/budget guard (per cost_profile: llm) ────────────────────────

def load_state() -> dict:
    if STATE_FILE.exists():
        try:
            return json.loads(STATE_FILE.read_text())
        except ValueError:
            pass
    return {"window": [], "day": time.strftime("%Y-%m-%d"), "day_count": 0}


def allow(state: dict, policy: dict) -> bool:
    now = time.time()
    state["window"] = [t for t in state["window"] if now - t < 60]
    if state.get("day") != time.strftime("%Y-%m-%d"):
        state["day"], state["day_count"] = time.strftime("%Y-%m-%d"), 0
    if len(state["window"]) >= policy.get("requests_per_min", 12):
        return False
    if state["day_count"] >= policy.get("daily_budget_requests", 2000):
        return False
    return True


def consume(state: dict):
    state["window"].append(time.time())
    state["day_count"] = state.get("day_count", 0) + 1
    STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
    STATE_FILE.write_text(json.dumps(state))


# ── executor: per_section_summaries ──────────────────────────────────

def run_per_section(pid: str, spec: dict, state: dict) -> str:
    article = REPO / "data/processed" / pid
    parsed = json.loads((article / "parsed" / "article.json").read_text())
    pol = spec["policy"]
    prompt = (REPO / spec["prompt_file"]).read_text()
    endpoint, model = binding_model(spec["model_binding"])
    prompt_sha = hashlib.sha256(prompt.encode()).hexdigest()

    sections = [
        s for s in (parsed.get("sections") or [])
        if len(s.get("text") or "") >= pol.get("min_section_chars", 200)
        and not re.search(r"reference|acknowledg|bibliograph", (s.get("title") or ""), re.I)
    ]
    sections_sha = hashlib.sha256(json.dumps(
        [{"id": s.get("section_id"), "n": len(s.get("text") or "")} for s in sections]
    ).encode()).hexdigest()

    # cache: pid + model + prompt_sha + sections fingerprint
    CACHE.mkdir(parents=True, exist_ok=True)
    cf = CACHE / f"{pid}.json"
    if cf.exists():
        rec = json.loads(cf.read_text())
        if rec.get("ok"):
            return _write_output(article, pid, rec, model, prompt_sha, spec)

    batches = [sections[i:i + pol.get("max_sections_per_request", 16)]
               for i in range(0, len(sections), pol.get("max_sections_per_request", 16))]
    results, ok_all = [], True
    for batch in batches:
        if not allow(state, pol):
            return "retry_later"
        user = "\n\n".join(
            f"[path_id={s.get('path_id')} section_id={s.get('section_id')} title={s.get('title','')}]\n{s.get('text','')}"
            for s in batch)
        body = json.dumps({
            "model": model,
            "messages": [{"role": "system", "content": prompt},
                         {"role": "user", "content": user[:24000]}],
            "temperature": 0.2, "max_tokens": 4000, "stream": False,
        }).encode()
        key = _router_key()
        req = urllib.request.Request(endpoint, data=body, headers={
            "Content-Type": "application/json",
            **({"Authorization": f"Bearer {key}"} if key else {})})
        try:
            with urllib.request.urlopen(req, timeout=240) as r:
                resp = json.loads(r.read().decode())
            content = (resp.get("choices") or [{}])[0].get("message", {}).get("content") or ""
            arr = extract_json_array(content)
            if arr is None:
                ok_all = False
                break
            results.extend(arr)
        except Exception:
            ok_all = False
            break
        consume(state)
        time.sleep(5)  # politeness inside rpm limit

    rec = {"pid": pid, "model": model, "prompt_sha256": prompt_sha,
           "sections_sha256": sections_sha, "ok": ok_all,
           "summaries": results, "sections_meta": [
               {"path_id": s.get("path_id"), "section_id": s.get("section_id"),
                "title": s.get("title")} for s in sections],
           "fetched_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    if ok_all:
        cf.write_text(json.dumps(rec, ensure_ascii=False))
    return _write_output(article, pid, rec, model, prompt_sha, spec)


def _write_output(article: Path, pid: str, rec: dict, model: str, prompt_sha: str, spec: dict) -> str:
    mp = article / "manifest.yaml"
    m = yaml.safe_load(mp.read_text())
    sm = m["artifacts"].get("summaries") or {}
    if not rec.get("ok"):
        sm["per_section"] = {"status": "failed", "reason": "llm_batch_failed"}
        m["artifacts"]["summaries"] = sm
        mp.write_text(yaml.safe_dump(m, sort_keys=False, allow_unicode=True))
        return "failed"

    # join from the ARTICLE (FS = truth), not from cache shape
    parsed = json.loads((article / "parsed" / "article.json").read_text())
    secs = parsed.get("sections") or []
    meta = {s.get("path_id"): s for s in secs}
    meta_dot = {str(s.get("path_id")).strip("."): s for s in secs}  # LLM may emit '2' for AST '2.'

    def _lookup(pid):
        return meta.get(pid) or meta_dot.get(str(pid).strip("."))
    out = [{
        "path_id": s.get("path_id"),
        "section_id": (_lookup(s.get("path_id")) or {}).get("section_id"),
        "title": (_lookup(s.get("path_id")) or {}).get("title"),
        "summary": s.get("summary"),
    } for s in rec["summaries"]]
    sdir = article / "summaries"
    sdir.mkdir(exist_ok=True)
    (sdir / "per_section.json").write_text(json.dumps(out, ensure_ascii=False, indent=1))
    sm["per_section"] = {
        "status": "built",
        "file": "summaries/per_section.json",
        "sha256": hashlib.sha256((sdir / "per_section.json").read_bytes()).hexdigest(),
        "built_at": rec["fetched_at"], "actor": model,
        "sections": len(out),
    }
    m["artifacts"]["summaries"] = sm
    mp.write_text(yaml.safe_dump(m, sort_keys=False, allow_unicode=True))
    return "built"


def set_status(article: Path, status: str):
    mp = article / "manifest.yaml"
    m = yaml.safe_load(mp.read_text())
    sm = m["artifacts"].get("summaries") or {}
    sm["per_section"] = {"status": status}
    m["artifacts"]["summaries"] = sm
    mp.write_text(yaml.safe_dump(m, sort_keys=False, allow_unicode=True))


def main(argv):
    ids = argv[1:]
    task = next(t for t in REGISTRY["tasks"] if t["id"] == "per_section_summaries")
    if not task.get("enabled", True):
        print("task disabled in registry"); return 0
    state = load_state()
    counts = {}
    for pid in ids:
        article = REPO / "data/processed" / pid
        sm = (yaml.safe_load((article / "manifest.yaml").read_text())
              .get("artifacts", {}).get("summaries", {}))
        status = (sm.get("per_section") or {}).get("status", "not_requested")
        if status == "built":
            r = "built(cached)"
        elif status == "not_requested":
            set_status(article, "requested")
            set_status(article, "building")
            r = run_per_section(pid, task, state)
        else:
            r = run_per_section(pid, task, state)
        counts[r] = counts.get(r, 0) + 1
        print(f"  {pid}: {r}")
    print(f"per_section: {counts} | rpm-window={len(state['window'])} day={state.get('day_count')}")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
