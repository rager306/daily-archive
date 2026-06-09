#!/usr/bin/env python3
"""Generate a derived project trajectory report from existing controls.

The report is not a source of truth. It composes GSD files, ADR docs,
governance mirrors, recent milestone summaries, README, git status, and an
optional codebase-memory MCP snapshot to help agents stay aligned with the
project trajectory.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "project-trajectory"
DIMENSIONS = ("architecture", "functionality", "module_code", "evidence", "safety", "operations", "next_gate")
PROHIBITED_PATTERNS = {
    "graph_import_authorized": re.compile(r"(?i)(graph import|GraphDB import|LadybugDB import).{0,80}(authorized|allowed|enabled)"),
    "fact_promotion_allowed": re.compile(r"(?i)(fact promotion|promoted facts).{0,80}(authorized|allowed|enabled)"),
    "production_import_authorized": re.compile(r"(?i)(production import).{0,80}(authorized|allowed|enabled)"),
    "raw_payload_promoted": re.compile(r"(?i)(raw TEI|raw text|full text|embedding|vector).{0,80}(persisted|promoted|imported)"),
}
NO_IMPORT_COUNTER_TERMS = (
    "no graph import",
    "not authorized",
    "not persisted",
    "not promoted",
    "not imported",
    "prohibited",
    "disabled",
    "false",
    "blocked",
    "before any",
    "only advance",
)


@dataclass(frozen=True)
class Paths:
    root: Path
    project: Path
    requirements: Path
    decisions: Path
    readme: Path
    governance_graph: Path
    governance_mirror: Path
    adr_dir: Path
    milestones_dir: Path
    artifacts_dir: Path

    @classmethod
    def from_root(cls, root: Path) -> Paths:
        return cls(
            root=root,
            project=root / ".gsd" / "PROJECT.md",
            requirements=root / ".gsd" / "REQUIREMENTS.md",
            decisions=root / ".gsd" / "DECISIONS.md",
            readme=root / "README.md",
            governance_graph=root / ".codebase-memory" / "governance-graph.json",
            governance_mirror=root / ".codebase-memory" / "adr.md",
            adr_dir=root / "doc" / "adr",
            milestones_dir=root / ".gsd" / "milestones",
            artifacts_dir=root / "artifacts",
        )


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8") if path.exists() else ""


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def parse_requirements(text: str) -> dict[str, Any]:
    ids = re.findall(r"^### (R\d{3}) — (.+)$", text, flags=re.MULTILINE)
    statuses = Counter(re.findall(r"^- Status: ([^\n]+)$", text, flags=re.MULTILINE))
    return {"count": len(ids), "ids": [rid for rid, _ in ids], "statuses": dict(statuses), "titles": dict(ids)}


def parse_decisions(text: str) -> dict[str, Any]:
    rows = []
    for line in text.splitlines():
        if not line.startswith("| D"):
            continue
        parts = [part.strip() for part in line.strip("|").split("|")]
        if len(parts) >= 5:
            rows.append({"id": parts[0], "when": parts[1], "scope": parts[2], "decision": parts[3], "choice": parts[4]})
    return {"count": len(rows), "latest": rows[-5:], "ids": [row["id"] for row in rows]}


def recent_milestones(milestones_dir: Path, limit: int = 6) -> list[dict[str, Any]]:
    summaries = sorted(milestones_dir.glob("M*/M*-SUMMARY.md"), key=lambda p: p.stat().st_mtime if p.exists() else 0)
    items = []
    for path in summaries[-limit:]:
        text = read_text(path)
        title_match = re.search(r"title:\s*\"?([^\n\"]+)", text)
        status_match = re.search(r"status:\s*([^\n]+)", text)
        milestone_id = path.parent.name
        items.append(
            {
                "id": milestone_id,
                "path": str(path.relative_to(ROOT)) if ROOT in path.resolve().parents else str(path),
                "title": title_match.group(1).strip() if title_match else milestone_id,
                "status": status_match.group(1).strip() if status_match else "summary-present",
                "mentions_no_import": "no graph import" in text.lower() or "no-write" in text.lower(),
            }
        )
    return items


def git_status(root: Path) -> dict[str, Any]:
    try:
        result = subprocess.run(["git", "status", "--short"], cwd=root, text=True, capture_output=True, timeout=10)
        lines = [line for line in result.stdout.splitlines() if line]
        return {"exit_code": result.returncode, "changed_files": len(lines), "entries": lines[:100]}
    except Exception as exc:  # noqa: BLE001 - diagnostic only
        return {"error": f"{type(exc).__name__}: {exc}", "changed_files": -1, "entries": []}


def governance_summary(graph: dict[str, Any]) -> dict[str, Any]:
    nodes = graph.get("nodes", []) if isinstance(graph, dict) else []
    edges = graph.get("edges", []) if isinstance(graph, dict) else []
    types = Counter(node.get("type") for node in nodes if isinstance(node, dict))
    return {"node_count": len(nodes), "edge_count": len(edges), "node_types": dict(types)}


def find_prohibited_claims(texts: dict[str, str]) -> list[dict[str, str]]:
    flags: list[dict[str, str]] = []
    for source, text in texts.items():
        for flag, pattern in PROHIBITED_PATTERNS.items():
            for match in pattern.finditer(text):
                local_prefix = text[max(0, match.start() - 24) : match.start()].lower()
                matched_phrase = text[match.start() : match.end()].lower()
                local_negation = re.search(r"(?:^|\b)(no|not)\s*$", local_prefix) is not None
                raw_payload_negation = flag == "raw_payload_promoted" and "no raw" in local_prefix
                if any(term in matched_phrase for term in NO_IMPORT_COUNTER_TERMS) or local_negation or raw_payload_negation:
                    continue
                window = text[max(0, match.start() - 120) : match.end() + 120]
                flags.append({"flag": flag, "source": source, "snippet": " ".join(window.split())[:240]})
    return flags


def dimension(status: str, evidence: list[str], flags: list[str] | None = None) -> dict[str, Any]:
    return {"status": status, "evidence": evidence, "flags": flags or []}


def build_report(*, root: Path = ROOT, codebase_memory_snapshot: Path | None = None) -> dict[str, Any]:
    paths = Paths.from_root(root)
    requirements_text = read_text(paths.requirements)
    decisions_text = read_text(paths.decisions)
    readme = read_text(paths.readme)
    graph = load_json(paths.governance_graph)
    reqs = parse_requirements(requirements_text)
    decisions = parse_decisions(decisions_text)
    milestones = recent_milestones(paths.milestones_dir)
    latest = milestones[-1] if milestones else {}
    git = git_status(root)
    governance = governance_summary(graph)
    cbm = load_json(codebase_memory_snapshot) if codebase_memory_snapshot else {}

    drift_flags: list[dict[str, Any]] = []
    if not governance.get("node_count"):
        drift_flags.append({"flag": "governance_mirror_missing", "severity": "high", "evidence": str(paths.governance_graph)})
    latest_short_id = latest.get("id", "").split("-", 1)[0] if latest else ""
    if latest and latest["id"] not in readme and latest_short_id not in readme:
        drift_flags.append({"flag": "latest_milestone_missing_readme_reference", "severity": "medium", "evidence": latest["id"]})
    prohibited = find_prohibited_claims({"README.md": readme, "recent_milestones": "\n".join(read_text(Path(item["path"])) for item in milestones if Path(item["path"]).exists())})
    drift_flags.extend({"flag": item["flag"], "severity": "high", "evidence": item["source"], "snippet": item["snippet"]} for item in prohibited)
    if "Next safe milestone" not in readme and "Next gate" not in readme and "next gate" not in readme.lower():
        drift_flags.append({"flag": "missing_next_gate", "severity": "medium", "evidence": "README.md"})
    if git.get("changed_files", 0) > 0:
        drift_flags.append({"flag": "uncommitted_changes_present", "severity": "info", "evidence": f"{git['changed_files']} files"})

    safety_flags = [flag["flag"] for flag in drift_flags if flag["severity"] == "high"]
    verdict = "blocked" if safety_flags else ("drift_risk" if any(flag["severity"] == "medium" for flag in drift_flags) else "on_track")

    dimensions = {
        "architecture": dimension(
            "tracked",
            [".gsd/DECISIONS.md", "doc/adr/", ".codebase-memory/governance-graph.json"],
            [flag["flag"] for flag in drift_flags if "architecture" in flag["flag"]],
        ),
        "functionality": dimension(
            "tracked",
            [f"requirements={reqs['count']}", f"statuses={reqs['statuses']}"],
            [],
        ),
        "module_code": dimension(
            "tracked" if git.get("exit_code", 0) == 0 else "unknown",
            [f"git_changed_files={git.get('changed_files')}"],
            ["uncommitted_changes_present"] if git.get("changed_files", 0) > 0 else [],
        ),
        "evidence": dimension(
            "tracked" if milestones else "missing",
            [item["path"] for item in milestones[-3:]],
            [],
        ),
        "safety": dimension(
            "clear" if not safety_flags else "blocked",
            ["prohibited-claim scan over PROJECT/README/recent summaries"],
            safety_flags,
        ),
        "operations": dimension(
            "tracked",
            ["runtime/service state is artifact-derived; live process management remains external"],
            [],
        ),
        "next_gate": dimension(
            "clear" if not any(flag["flag"] == "missing_next_gate" for flag in drift_flags) else "needs_attention",
            ["README.md", "recent milestone summaries"],
            [flag["flag"] for flag in drift_flags if flag["flag"] == "missing_next_gate"],
        ),
    }

    report = {
        "schema_version": "m045.project-trajectory.v1",
        "verdict": verdict,
        "dimensions": dimensions,
        "drift_flags": drift_flags,
        "progress_summary": {
            "recent_milestones": milestones,
            "latest_milestone": latest,
            "requirements": reqs,
            "decisions": {"count": decisions["count"], "latest": decisions["latest"]},
            "governance_graph": governance,
        },
        "codebase_memory": {
            "provided": bool(cbm),
            "canonical": False,
            "snapshot": cbm,
        },
        "next_actions": derive_next_actions(drift_flags, latest),
        "derived_not_canonical": True,
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    return report


def derive_next_actions(drift_flags: list[dict[str, Any]], latest: dict[str, Any]) -> list[str]:
    actions = []
    names = {flag["flag"] for flag in drift_flags}
    if "governance_mirror_missing" in names:
        actions.append("Regenerate governance mirror before planning.")
    if "latest_milestone_missing_readme_reference" in names and latest:
        actions.append(f"Update README with latest milestone {latest['id']} interpretation.")
    if "missing_next_gate" in names:
        actions.append("Add or confirm the next gate before starting broad implementation.")
    if "uncommitted_changes_present" in names:
        actions.append("Run focused verification and commit or intentionally leave a handoff.")
    if not actions:
        actions.append("Proceed with next planned gate; keep using trajectory check before planning and closeout.")
    return actions


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Project Trajectory Report",
        "",
        f"- Verdict: `{report['verdict']}`",
        f"- Derived, not canonical: {str(report['derived_not_canonical']).lower()}",
        "- Graph writes: disabled",
        "- Production import: disabled",
        "- Fact promotion: disabled",
        f"- codebase-memory snapshot provided: {str(report['codebase_memory']['provided']).lower()}",
        "",
        "## Dimensions",
        "",
        "| Dimension | Status | Flags | Evidence |",
        "|---|---|---|---|",
    ]
    for name, data in report["dimensions"].items():
        lines.append(f"| {name} | {data['status']} | {', '.join(data['flags']) or 'none'} | {', '.join(data['evidence'])[:180]} |")
    lines.extend(["", "## Drift flags", "", "| Severity | Flag | Evidence |", "|---|---|---|"])
    for flag in report["drift_flags"]:
        lines.append(f"| {flag['severity']} | {flag['flag']} | {flag.get('evidence', '')} |")
    if not report["drift_flags"]:
        lines.append("| info | none | No drift flags detected |")
    lines.extend(["", "## Recent milestones", "", "| Milestone | Title | Status |", "|---|---|---|"])
    for item in report["progress_summary"]["recent_milestones"]:
        lines.append(f"| {item['id']} | {item['title']} | {item['status']} |")
    lines.extend(["", "## Next actions", ""])
    for action in report["next_actions"]:
        lines.append(f"- {action}")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--codebase-memory-snapshot", type=Path)
    args = parser.parse_args()
    report = build_report(root=args.root, codebase_memory_snapshot=args.codebase_memory_snapshot)
    write_json(args.output_dir / "trajectory-report.json", report)
    write_text(args.output_dir / "trajectory-report.md", render_markdown(report))
    sys.stdout.write(f"trajectory report: verdict={report['verdict']} flags={len(report['drift_flags'])}\n")
    return 0 if report["verdict"] != "blocked" else 2


if __name__ == "__main__":
    raise SystemExit(main())
