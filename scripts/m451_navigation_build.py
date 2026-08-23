#!/usr/bin/env python3
"""M451 v10: context/navigation.json — PageIndex semantics restored.

M025 Python-era PageIndex ideas (parent/children/next/path + find_by_title,
children_of, path_to, walk_next) were lost in the flat Rust projection.
Node identity is section_id (path_id duplicates in 55% of papers);
parent derives from the path_id grammar (dot hierarchy, trailing-dot
headers are metadata nodes, tilde restarts never become parents).
Queries below are the reference implementation for the Rust side.
"""
from __future__ import annotations

import hashlib
import re
import json
import sys
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parent.parent


def parent_path(path_id: str):
    """'1.2.3'->'1.2'; '1.2'->'1'; '1'->None; '1.'->None (header node)."""
    p = str(path_id).strip()
    if p.endswith("."):
        return None
    if "." not in p:
        return None
    parent = p.rsplit(".", 1)[0]
    return parent or None


def _load_detectors() -> list:
    try:
        cfg = yaml.safe_load(
            (REPO / "data/ontology/furniture_detectors.yaml").read_text()) or {}
        return cfg.get("detectors", [])
    except OSError:
        return []


def _tree_cost(nodes: list) -> dict:
    """Vectify model adapted: S=node char_count, R from policy DATA."""
    try:
        pol = yaml.safe_load(
            (REPO / "data/ontology/navigation_quality_policy.yaml").read_text()) or {}
        R = pol.get("model", {}).get("routing_cost", 250)
        th = pol.get("signal", {}).get("thresholds", {})
    except OSError:
        R, th = 250, {}
    by = {n["section_id"]: n for n in nodes}
    children = {}
    for n in nodes:
        p = n.get("parent_section_id")
        if p:
            children.setdefault(p, []).append(n["section_id"])

    def size(nid):
        return by[nid].get("char_count", 0)

    def cost(nid, d):
        kids = children.get(nid, [])
        if not kids:
            return d * R + size(nid)
        child_sum = sum(size(k) for k in kids)
        residual = max(0, size(nid) - child_sum)
        branches = [cost(k, d + 1) for k in kids]
        if residual > 0:
            branches.append((d + 1) * R + residual)
        return max(branches) if branches else d * R + size(nid)

    roots = [n["section_id"] for n in nodes if not n.get("parent_section_id")]
    worst = max((cost(r, 1) for r in roots), default=0)
    return {
        "tree_cost": worst,
        "max_depth": max((n["depth"] for n in nodes), default=0),
        "frontier_nodes": sum(1 for n in nodes if not children.get(n["section_id"])),
        "thresholds": th,
    }


def build_navigation(pid: str, root: Path) -> dict:
    article = root / pid
    proj = json.loads((article / "context" / "projection.json").read_text())
    secs = proj.get("sections") or []
    by_sid = {s["section_id"]: s for s in secs}
    by_path = {}
    for s in secs:
        by_path.setdefault(str(s["path_id"]), s)  # first wins on path dupes

    # v11: role detectors (DATA) + char_count (size for tree_cost)
    detectors = _load_detectors()

    def classify(title, pid_path):
        t = (title or "").strip()
        for d in detectors:
            pat = d.get("title_pattern")
            if pat and re.match(pat, t):
                return d.get("role", "body"), d.get("confidence", "heuristic")
        if d_ids and pid_path.endswith("."):
            return "header_metadata", "deterministic"
        return "body", "heuristic"

    d_ids = []
    nodes = []
    for s in sorted(secs, key=lambda x: x.get("order", 0)):
        pid_path = str(s["path_id"])
        pp = parent_path(pid_path)
        parent = by_path.get(pp) if pp else None
        depth = 0 if pid_path.endswith(".") else (pid_path.count(".") + 1 if "." in pid_path else 1)
        role, conf = classify(s.get("title"), pid_path)
        nodes.append({
            "section_id": s["section_id"],
            "path_id": pid_path,
            "depth": depth,
            "parent_section_id": parent["section_id"] if parent else None,
            "children_section_ids": [],  # filled below
            "prev_section_id": None,
            "next_section_id": None,
            "title": s.get("title"),
            "char_count": s.get("char_count", 0),
            "role": role,
            "role_confidence": conf,
        })
        d_ids.append(s["section_id"])
    by_nid = {n["section_id"]: n for n in nodes}
    # children (document order preserved by nodes list)
    for n in nodes:
        if n["parent_section_id"]:
            by_nid[n["parent_section_id"]]["children_section_ids"].append(n["section_id"])
    # prev/next in document order (PageIndex walk semantics)
    for a, b in zip(nodes, nodes[1:]):
        a["next_section_id"] = b["section_id"]
        b["prev_section_id"] = a["section_id"]

    (article / "context" / "navigation.json").write_text(
        json.dumps(nodes, ensure_ascii=False, indent=1))

    mp = article / "manifest.yaml"
    m = yaml.safe_load(mp.read_text())
    tc = _tree_cost(nodes)
    furniture = sum(1 for n in nodes if n["role"] == "furniture")
    share = furniture / max(len(nodes), 1)
    m["artifacts"]["context"]["navigation"] = {
        "file": "context/navigation.json",
        "sha256": hashlib.sha256((article / "context" / "navigation.json").read_bytes()).hexdigest(),
        "actor": "m451-navigation-builder",
        "stats": {
            "nodes": len(nodes),
            "roots": sum(1 for n in nodes if not n["parent_section_id"]),
            "max_depth": max(n["depth"] for n in nodes),
            "path_dupes": len(nodes) - len({n["path_id"] for n in nodes}),
            "furniture": furniture,
            "furniture_share": round(share, 3),
            "tree_cost": tc["tree_cost"],
        },
        "inputs": ["data/ontology/furniture_detectors.yaml",
                   "data/ontology/navigation_quality_policy.yaml"],
    }
    # v11 signals appended to quality_signals.json (merge, keep others)
    qs_path = article / "quality_signals.json"
    qs = json.loads(qs_path.read_text()) if qs_path.exists() else []
    qs = [s for s in qs if s.get("signal_id") not in ("navigation_efficiency", "furniture_share")]
    def verdict(value, th, higher_bad=True):
        if not th: return "info"
        for k in ("warn_above", "alarm_above"):
            if k in th and value > th[k]:
                return "alarm" if k == "alarm_above" else "warn"
        return "ok"
    nth = tc.get("thresholds", {})
    qs.append({"signal_id": "navigation_efficiency", "value": tc["tree_cost"],
               "scale": "chars", "verdict": verdict(tc["tree_cost"], nth)})
    qs.append({"signal_id": "furniture_share", "value": round(share, 3),
               "scale": "ratio", "verdict": verdict(share, {"warn_above": 0.25, "alarm_above": 0.5})})
    qs_path.write_text(json.dumps(qs, ensure_ascii=False, indent=1))
    mp.write_text(yaml.safe_dump(m, sort_keys=False, allow_unicode=True))
    return {"nodes": len(nodes)}


# ── reference queries (PageIndex API) ────────────────────────────────

def load_nav(pid: str) -> list[dict]:
    return json.loads((REPO / "data/processed" / pid / "context" / "navigation.json").read_text())


def find_by_title(pid: str, needle: str) -> list[dict]:
    n = needle.lower()
    return [x for x in load_nav(pid) if n in (x.get("title") or "").lower()]


def children_of(pid: str, section_id: str) -> list[dict]:
    return [x for x in load_nav(pid) if x["section_id"] in
            next(n["children_section_ids"] for n in load_nav(pid) if n["section_id"] == section_id)]


def path_to_root(pid: str, section_id: str) -> list[str]:
    by = {n["section_id"]: n for n in load_nav(pid)}
    chain, cur = [], section_id
    while cur and cur in by:
        chain.append(cur)
        cur = by[cur]["parent_section_id"]
    return chain


def walk_next(pid: str, section_id: str):
    by = {n["section_id"]: n for n in load_nav(pid)}
    nid = by[section_id].get("next_section_id")
    return by.get(nid) if nid else None


if __name__ == "__main__":
    root = REPO / "data/processed"
    ids = sys.argv[1:] or sorted(d.name for d in root.iterdir()
                                 if (d / "context" / "projection.json").exists())
    total = 0
    for pid in ids:
        r = build_navigation(pid, root)
        total += r["nodes"]
    print(f"navigation built: {len(ids)} articles, {total} nodes")


def _selfcheck(pid: str) -> list[str]:
    errs = []
    nav = load_nav(pid)
    by = {n["section_id"]: n for n in nav}
    for n in nav:
        if n["parent_section_id"] and n["parent_section_id"] not in by:
            errs.append(f"dangling parent {n['section_id']}")
        if n["next_section_id"] and by.get(n["next_section_id"], {}).get("prev_section_id") != n["section_id"]:
            errs.append(f"next/prev asymmetric {n['section_id']}")
    return errs
