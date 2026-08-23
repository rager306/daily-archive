#!/usr/bin/env python3
"""M451 summaries layer: LLM comparison with deterministic metrics.

All policy from data/ontology/summary_eval_policy.yaml (DATA).
Known 9router gotchas handled: JSON wrapped in prose/fences,
<think> reasoning blocks, empty bodies. Cache-first, replay-safe.
"""
from __future__ import annotations

import hashlib
import json
import re
import statistics
import sys
import time
import urllib.request
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parent.parent
POLICY_PATH = REPO / "data/ontology/summary_eval_policy.yaml"
CACHE = REPO / "artifacts/pipeline_cache/summaries"
OUT = REPO / "artifacts/summaries_eval"

STOP = set("""a an the of to in on for with and or is are was were be been this that these those
we our it its as by at from not no than then so such can could may might will would shall should
have has had do does did but if when where which who whom whose what how why all any both each
few more most other some only own same too very just also into over under between during
through above below out off again further once here there""".split())


def norm_words(text: str) -> list[str]:
    return [w for w in re.findall(r"[a-z][a-z\-]{2,}", text.lower()) if w not in STOP]


def content_set(text: str) -> set[str]:
    return set(norm_words(text))


# ── LLM call (cache-first, hardened extraction) ──────────────────────

def extract_json(raw: str):
    """Best-effort JSON from model output: strip <think>, fences, prose."""
    raw = re.sub(r"<think>.*?</think>", "", raw, flags=re.S)
    raw = re.sub(r"^```(?:json)?\s*|\s*```$", "", raw.strip())
    try:
        return json.loads(raw), "direct"
    except ValueError:
        pass
    m = re.search(r"\{.*\}", raw, re.S)
    if m:
        try:
            return json.loads(m.group(0)), "extracted"
        except ValueError:
            pass
    return None, "unparseable"


def _router_key() -> str:
    """NINE_ROUTER_API_KEY from env or .env (never logged)."""
    import os
    key = os.environ.get("NINE_ROUTER_API_KEY", "")
    if not key:
        envf = REPO / ".env"
        if envf.exists():
            for ln in envf.read_text().splitlines():
                if ln.startswith("NINE_ROUTER_API_KEY="):
                    key = ln.split("=", 1)[1].strip().strip('"').strip("'")
                    break
    return key


def call_model(policy: dict, model, prompt: str, user: str, use_cache: bool = True):
    """model: str name or {name, label, extra} spec from policy (DATA)."""
    label = model
    extra = {}
    if isinstance(model, dict):
        label = model.get("label") or model["name"]
        extra = model.get("extra") or {}
        model = model["name"]
    slug = re.sub(r"[^a-z0-9]+", "-", label.lower()).strip("-")
    CACHE.mkdir(parents=True, exist_ok=True)
    fingerprint = model + json.dumps(extra, sort_keys=True) + user[:200]
    key = hashlib.sha256(fingerprint.encode()).hexdigest()[:16]
    cf = CACHE / f"{slug}-{key}.json"
    if use_cache and cf.exists():
        rec = json.loads(cf.read_text())
        if rec.get("done") and rec.get("ok"):
            return rec

    key = _router_key()
    body_d = {
        "model": model,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": user},
        ],
        "temperature": 0.2,
        "max_tokens": 8000,  # headroom: effort modes spend more on reasoning
        "stream": False,
    }
    body_d.update(extra)  # per-model DATA overrides (e.g. reasoning_effort)
    body = json.dumps(body_d).encode()
    headers = {"Content-Type": "application/json"}
    if key:
        headers["Authorization"] = f"Bearer {key}"
    rec = {"model": label, "model_name": model, "prompt_key": cf.stem, "ok": False, "done": False}
    # NOTE: `done` flips to True ONLY with ok=True (parsed success).
    # Failures (HTTP/network) stay retryable — never cached as terminal.
    last_err = None
    for attempt in range(3):
        req = urllib.request.Request(policy["router"]["endpoint"], data=body, headers=headers)
        t0 = time.time()
        try:
            with urllib.request.urlopen(req, timeout=policy["router"]["timeout_s"]) as r:
                resp = json.loads(r.read().decode())
            content = (resp.get("choices") or [{}])[0].get("message", {}).get("content") or ""
            if not content:
                content = (resp.get("choices") or [{}])[0].get("message", {}).get("reasoning_content") or ""
            obj, how = extract_json(content)
            rec.update({"ok": obj is not None, "parsed": obj, "extraction": how,
                        "raw_len": len(content), "latency_s": round(time.time() - t0, 1),
                        "usage": resp.get("usage", {})})
            last_err = None
            break
        except urllib.error.HTTPError as e:
            last_err = f"HTTP {e.code}"
            if e.code in (429, 500, 502, 503):
                time.sleep(20 * (attempt + 1))  # backoff for rate-limited free channels
                continue
            break
        except Exception as e:
            last_err = str(e)[:160]
            break
    if last_err and not rec["ok"]:
        rec.update({"ok": False, "error": last_err})
    rec["done"] = rec["ok"]
    cf.write_text(json.dumps(rec, ensure_ascii=False))
    return rec


# ── deterministic metrics ─────────────────────────────────────────────

def summary_text(obj: dict) -> str:
    parts = [str(obj.get("tldr", "")), str(obj.get("methods", "")),
             str(obj.get("results", "")), str(obj.get("limitations", ""))]
    parts += [str(c) for c in (obj.get("contributions") or [])]
    return " ".join(parts)


def metrics_for(policy: dict, obj, article_words: set, entities: list, article_text: str) -> dict:
    m = {}
    fields = policy["output_fields"]
    s = summary_text(obj or {})

    # structure
    filled = sum(1 for f in fields if obj.get(f))
    tldr = str(obj.get("tldr", ""))
    contributions = obj.get("contributions") or []
    max_tldr = policy["metrics"][0].get("params", {}).get("max_tldr_chars", 500)
    m["structure_compliance"] = float(
        filled == len(fields)
        and 0 < len(tldr) <= max_tldr
        and len(contributions) >= 2
    )

    # grounding: per-sentence content overlap vs article
    min_ov = policy["metrics"][1].get("params", {}).get("min_overlap", 0.30)
    sents = [x for x in re.split(r"(?<=[.!?])\s+", s.strip()) if len(x) > 30]
    grounded = 0
    for sent in sents:
        cw = set(norm_words(sent))
        if not cw:
            continue
        ov = len(cw & article_words) / len(cw)
        if ov >= min_ov:
            grounded += 1
    m["grounding_claims"] = round(grounded / max(len(sents), 1), 3)

    # entity coverage
    if entities:
        hit = sum(1 for e in entities if e.lower() in s.lower())
        m["entity_coverage"] = round(hit / len(entities), 3)
    else:
        m["entity_coverage"] = None

    # numeric fidelity
    nums = set(re.findall(r"\d+(?:\.\d+)?%?", s))
    if nums:
        found = sum(1 for n in nums if n in article_text)
        m["numeric_fidelity"] = round(found / len(nums), 3)
    else:
        m["numeric_fidelity"] = None

    # hallucination proxy: capitalized multiword names not in article
    names = set(re.findall(r"\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+){1,2})\b", s))
    bad = [n for n in names if n.lower() not in article_text.lower()]
    m["hallucination_proxy"] = round(len(bad) / max(len(names), 1), 3) if names else None

    m["compression"] = round(len(s) / max(len(article_text), 1), 4)
    m["summary_chars"] = len(s)
    return m


# ── runner ────────────────────────────────────────────────────────────

def build_user(policy: dict, parsed: dict) -> str:
    maxc = policy["input"]["max_body_chars"]
    secs = parsed.get("sections") or []
    body = "\n\n".join(f"## {s.get('title','')}\n{s.get('text','')}" for s in secs)
    text = f"TITLE: {parsed.get('title','')}\n\nABSTRACT: {parsed.get('abstract_text','')}\n\n{body}"
    if len(text) > maxc:
        text = text[:maxc] + "\n...[truncated]"
    return text


def main(argv):
    policy = yaml.safe_load(POLICY_PATH.read_text())
    # Candidate mode: --prompt-file <path> swaps the system prompt for this
    # eval run ONLY (baseline in prod stays untouched). Cache fingerprint
    # includes the prompt sha, so runs never mix.
    args = [a for a in argv[1:]]
    prompt_file = None
    if "--prompt-file" in args:
        i = args.index("--prompt-file")
        prompt_file = args[i + 1]
        del args[i:i + 2]
        pf = Path(prompt_file)
        policy = dict(policy)
        policy["prompt"] = pf.read_text()
        policy.setdefault("label_suffix", "")
        policy["label_suffix"] = f" [prompt:{hashlib.sha256(policy['prompt'].encode()).hexdigest()[:8]}]"
        pids = args
    pids = args or ["2402.18361", "2408.06292", "1203.2295", "2201.11903", "1712.00600", "2503.22732"]
    models = policy["models"]

    rows = []
    from concurrent.futures import ThreadPoolExecutor

    def mlabel(m):
        return m.get("label") or m if isinstance(m, dict) else m

    jobs = []
    for pid in pids:
        parsed = json.loads((REPO / f"data/processed/{pid}/parsed/article.json").read_text())
        user = build_user(policy, parsed)
        for model in models:
            jobs.append((pid, model, parsed, user))

    def work(job):
        pid, model, parsed, user = job
        rec = call_model(policy, model, policy["prompt"], user)
        ent_file = REPO / f"data/processed/{pid}/extracted/entities.json"
        entities = []
        if ent_file.exists():
            entities = [e.get("label", "") for e in json.loads(ent_file.read_text())]
        article_text = user
        aw = content_set(article_text)
        if rec.get("ok"):
            mets = metrics_for(policy, rec["parsed"], aw, entities, article_text)
        else:
            mets = None
        return {"paper": pid, "model": rec.get("model", mlabel(model)), "ok": rec["ok"],
                "extraction": rec.get("extraction"), "latency_s": rec.get("latency_s"),
                "error": rec.get("error"), "metrics": mets}

    with ThreadPoolExecutor(max_workers=6) as ex:
        rows = list(ex.map(work, jobs))

    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "rows.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1))

    # aggregate per model
    print(f"\n{'model':34} {'ok':>3} {'struct':>7} {'ground':>7} {'entity':>7} {'num':>6} {'halluc':>7} {'med_lat':>8}")
    print("-" * 88)
    agg = {}
    for model in map(mlabel, models):
        rs = [r for r in rows if r["model"] == model]
        ok = [r for r in rs if r["ok"]]
        def med(k):
            vals = [r["metrics"][k] for r in ok if r["metrics"] and r["metrics"].get(k) is not None]
            return statistics.median(vals) if vals else float("nan")
        lat = statistics.median([r["latency_s"] for r in ok]) if ok else float("nan")
        agg[model] = {"ok": len(ok), "total": len(rs), "struct": med("structure_compliance"),
                      "ground": med("grounding_claims"), "entity": med("entity_coverage"),
                      "num": med("numeric_fidelity"), "halluc": med("hallucination_proxy"),
                      "latency": lat}
        a = agg[model]
        def f(x):
            return f"{x:.2f}" if x == x else "  -"
        print(f"{model:34} {a['ok']:>2}/{a['total']:<2} {f(a['struct']):>7} {f(a['ground']):>7} "
              f"{f(a['entity']):>7} {f(a['num']):>6} {f(a['halluc']):>7} {lat:>7.1f}s")
    (OUT / "aggregate.json").write_text(json.dumps(agg, ensure_ascii=False, indent=1))
    print(f"\nreport: {OUT}/aggregate.json")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
