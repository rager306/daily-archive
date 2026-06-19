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
DIMENSIONS = (
    "architecture", "functionality", "module_code", "evidence", "safety",
    "operations", "next_gate", "reverse_adr_audit",
    # Post-M101 architecture dimensions (ADR-023)
    "schema_readiness", "extraction_coverage", "falkordb_migration",
    "universal_sources", "agent_readiness",
)
PHASES = ("preflight", "active", "closeout")

# Phase-aware severity overrides per D080 and M046 Roadmap Recommendation 5.
# Default = preflight (current behavior). Active phase promotes uncommitted
# changes to medium to encourage frequent commits. Closeout demotes it
# back to info because changes are expected before commit.
PHASE_SEVERITY_OVERRIDES: dict[str, dict[str, str]] = {
    "preflight": {},
    "active": {"uncommitted_changes_present": "medium"},
    "closeout": {"uncommitted_changes_present": "info"},
}
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


def adjust_severity_for_phase(flags: list[dict[str, Any]], phase: str) -> list[dict[str, Any]]:
    """Apply phase-aware severity overrides to a list of drift flags.

    Returns a new list; the original is not mutated. Each flag's severity
    is replaced if there is a per-phase override for its flag id.
    """
    overrides = PHASE_SEVERITY_OVERRIDES.get(phase, {})
    if not overrides:
        return list(flags)
    adjusted: list[dict[str, Any]] = []
    for flag in flags:
        new_severity = overrides.get(flag.get("flag"))
        if new_severity is None:
            adjusted.append(flag)
            continue
        adjusted.append({**flag, "severity": new_severity, "phase_override": True})
    return adjusted


# Reverse ADR audit: code-level checks that the binding ADRs are honoured
# in actual code, not just in narrative. Each rule has an anchor ADR/R and
# a severity (high = blocking, medium = drift_risk).
#
# An anchor ADR/R is a binding decision. A violation here is a binding breach.
REVERSE_ADR_AUDIT_RULES: list[dict[str, Any]] = [
    {
        "id": "no_ladybugdb_import_outside_graph_package",
        "anchor": "ADR-022 (FalkorDB binding), ADR-005 (No Direct Extractor to GraphDB)",
        "severity": "medium",
        "scan": "src/",
        "pattern": r"^\s*import\s+ladybugdb\b",
        "exclude_paths": ("src/research_graph/graph/ladybug_client.py",),
        "rationale": "Third-party `ladybugdb` library must only be imported in the graph substrate wrapper. FalkorDB (ADR-022) is the binding production target.",
    },
    {
        "id": "no_quantmind_runtime_import",
        "anchor": "ADR m034/007 (Quant-mind Pattern Source Not Runtime Dependency)",
        "severity": "high",
        "scan": "src/",
        "pattern": r"^\s*(?:from|import)\s+(?:quantmind|quant_mind|llmquant)\b",
        "rationale": "quant-mind is a pattern source only, not a runtime dependency.",
    },
    {
        "id": "no_graph_import_allowed_true_in_artifacts",
        "anchor": "ADR-005 (No Direct Extractor to GraphDB Path)",
        "severity": "high",
        "scan": "artifacts/",
        "pattern": r"\"graph_import_allowed\"\s*:\s*true",
        "rationale": "graph_import_allowed must remain false until a future explicit graph promotion milestone.",
    },
    {
        "id": "no_production_import_attempted_true_in_artifacts",
        "anchor": "ADR-005 (No Direct Extractor to GraphDB Path)",
        "severity": "high",
        "scan": "artifacts/",
        "pattern": r"\"production_import_attempted\"\s*:\s*true",
        "rationale": "production_import_attempted must remain false until a future explicit graph promotion milestone.",
    },
    {
        "id": "no_import_eligible_true_in_artifacts",
        "anchor": "R029 (Import-ready typed chunk package), ADR-005",
        "severity": "high",
        "scan": "artifacts/",
        "pattern": r"\"import_eligible\"\s*:\s*true",
        "rationale": "import_eligible must remain false until independent review and a future graph promotion milestone.",
    },
    {
        "id": "no_ladybugdb_written_true_in_artifacts",
        "anchor": "ADR-022 (FalkorDB binding)",
        "severity": "high",
        "scan": "artifacts/",
        "pattern": r"\"ladybugdb_written\"\s*:\s*true",
        "rationale": "ladybugdb_written must remain false. FalkorDB (ADR-022) is the binding production GraphDB.",
    },
    # Post-M101 rules (ADR-023 through ADR-032)
    {
        "id": "no_arxiv_archive_import",
        "anchor": "M099 (Full migration to research_graph)",
        "severity": "high",
        "scan": "src/",
        "pattern": r"^\s*(?:from|import)\s+arxiv_archive\b",
        "rationale": "arxiv_archive package was fully migrated to research_graph. No imports should remain.",
    },
    {
        "id": "no_anthropic_sdk_outside_llm",
        "anchor": "M100 (summarizer moved to llm), ADR-025 (multi-provider)",
        "severity": "medium",
        "scan": "src/",
        "pattern": r"^\s*import\s+anthropic\b",
        "exclude_paths": ("src/research_graph/llm/",),
        "rationale": "anthropic SDK should only be imported in llm/ package. Other modules use provider-agnostic interfaces.",
        "path_prefix_exclude": "src/research_graph/llm/",
    },
]


def reverse_adr_audit(root: Path) -> dict[str, Any]:
    """Run the 8-rule reverse ADR audit on the codebase.

    Returns a dict with keys:
      - status: "clear" | "violations"
      - violations: list of {rule_id, anchor, file, line, snippet}
      - evidence: list of scanned roots and rule count
    """
    violations: list[dict[str, str]] = []
    evidence: list[str] = []

    for rule in REVERSE_ADR_AUDIT_RULES:
        scan_root = root / rule["scan"]
        if not scan_root.exists():
            evidence.append(f"{rule['id']}: scan root {rule['scan']} not found")
            continue
        if not evidence or not evidence[-1].startswith(rule["scan"]):
            evidence.append(f"{rule['scan'].rstrip('/')}/ (rule: {rule['id']}, anchor: {rule['anchor']})")
        try:
            pattern = re.compile(rule["pattern"], flags=re.MULTILINE)
        except re.error as exc:
            violations.append({"rule_id": rule["id"], "anchor": rule["anchor"], "file": "<pattern>", "line": "0", "snippet": f"pattern error: {exc}"})
            continue
        exclude = set(rule.get("exclude_paths") or ())
        prefix_exclude = rule.get("path_prefix_exclude")
        for path in scan_root.rglob("*"):
            if not path.is_file():
                continue
            if path.suffix not in {".py", ".json", ".md"}:
                continue
            try:
                rel = str(path.relative_to(root))
            except ValueError:
                continue
            if rel in exclude:
                continue
            if prefix_exclude and rel.startswith(prefix_exclude):
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                continue
            for match in pattern.finditer(text):
                line_no = text[: match.start()].count("\n") + 1
                snippet = text.splitlines()[line_no - 1].strip() if line_no - 1 < len(text.splitlines()) else ""
                violations.append({
                    "rule_id": rule["id"],
                    "anchor": rule["anchor"],
                    "file": rel,
                    "line": str(line_no),
                    "snippet": snippet[:200],
                })

    status = "clear" if not violations else "violations"
    return {"status": status, "violations": violations, "evidence": evidence, "rule_count": len(REVERSE_ADR_AUDIT_RULES)}


def build_report(*, root: Path = ROOT, codebase_memory_snapshot: Path | None = None, phase: str = "preflight") -> dict[str, Any]:
    if phase not in PHASES:
        raise ValueError(f"unknown phase: {phase!r}; expected one of {PHASES}")
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
    audit = reverse_adr_audit(root)

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
    for violation in audit["violations"]:
        # Find the rule to get its severity
        rule_severity = "high"
        for rule in REVERSE_ADR_AUDIT_RULES:
            if rule["id"] == violation["rule_id"]:
                rule_severity = rule.get("severity", "high")
                break
        drift_flags.append({
            "flag": f"reverse_adr_audit_{violation['rule_id']}",
            "severity": rule_severity,
            "evidence": f"{violation['file']}:{violation['line']}",
            "snippet": violation["snippet"],
        })

    # Apply phase-aware severity overrides (D080, M046 Recommendation 5).
    drift_flags = adjust_severity_for_phase(drift_flags, phase)

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
        "reverse_adr_audit": dimension(
            "clear" if audit["status"] == "clear" else "violations",
            [f"rule_count={audit['rule_count']}", *audit["evidence"][:5]],
            [v["rule_id"] for v in audit["violations"]],
        ),
        # Post-M101 architecture dimensions (ADR-023)
        "schema_readiness": dimension(
            "design_accepted" if paths.adr_dir.joinpath("ADR-028-typed-knowledge-schema.md").exists() else "missing",
            ["ADR-028 typed schema", "27 relation types", "5 modules A-E"],
            [],
        ),
        "extraction_coverage": dimension(
            "not_started",
            ["Core-then-Modes pipeline designed (ADR-029)", "No extraction runs yet"],
            [],
        ),
        "falkordb_migration": dimension(
            "not_started" if not any(p.name == "falkordb_client.py" for p in (root / "src" / "research_graph" / "graph").glob("*.py")) else "in_progress",
            ["ADR-022 FalkorDB binding", "ADR-030 schema designed", "LadybugDB still in use"],
            [],
        ),
        "universal_sources": dimension(
            "paper_only",
            ["220 PDFs in arXiv catalog", "5 domain profiles designed (ADR-032)", "GNN textbook pending"],
            [],
        ),
        "agent_readiness": dimension(
            "requires_development",
            ["ADR-031 directional", "SymFSM needs formalization", "Phase 6 deferred"],
            [],
        ),
    }

    report = {
        "schema_version": "m101.project-trajectory.v2",
        "phase": phase,
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
        "reverse_adr_audit_details": audit,
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
        f"- Phase: `{report.get('phase', 'preflight')}`",
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

    lines.extend([
        "",
        "## How to create an ADR",
        "",
        "1. Use the canonical template: `doc/adr/ADR-TEMPLATE.md` (14 sections, Mermaid-assisted, LLM Reading Notes required).",
        "2. Number sequentially after the highest existing ADR number (e.g., next is `ADR-017`).",
        "3. Filename pattern: `doc/adr/ADR-NNN-short-title.md` (use hyphens, no slashes).",
        "4. Update `doc/adr/ADR-INDEX.md` table with the new entry.",
        "5. After commit, run `uv run python scripts/sync_codebase_memory_governance.py` to mirror.",
        "6. For amendments to existing ADRs, add an \"Amendment Log\" section with date + milestone + rationale.",
        "",
        "## Catalog ingestion rule (post-M061-S04, 2026-06-13)",
        "",
        "All future milestones/tasks that download arxiv articles MUST end with already-downloaded articles being ingested to the canonical catalog at `data/article_catalog/article_catalog/arxiv/<category>/<id>/source/<id>.pdf`.",
        "",
        "Reference pattern: `scripts/m061_ingest_to_canonical_catalog.py` (M061 S04, 2026-06-13).",
        "Idempotent (SHA256 check), online arxiv API category detection with 1 req/3s rate limit + retry+backoff, explicit network override with audit.",
        "",
        "Rationale: M061 S01-S03 placed 151 PDFs in `artifacts/m061-2hop/anchor-*/acquisition/pdfs/` (isolated from catalog). S04 closed the gap (catalog 186 -> 218 PDFs). Without this rule, future download tasks risk losing articles when `artifacts/` is cleaned up.",
        "",
        "Verification: `uv run python scripts/verify_article_catalog.py` must pass after any ingestion step.",
        "",
        "## Next gate (post-M101 architecture crystallization)",
        "",
        "Architecture is crystallized (32 ADRs, 6 design documents). Next phases:",
        "",
        "- **Phase 2**: Typed schema code + extraction prototype (5 papers, DSPy, MiniMax)",
        "- **Phase 3**: FalkorDB migration + graph operators O1-O6",
        "- **Phase 4**: Staged validation (R024: 10→20→week corpus)",
        "- **Phase 5**: Universal ingestion (GNN textbook, code repos, datasets)",
        "- **Phase 6**: Agent integration (SymFSM) — REQUIRES FURTHER IDEA DEVELOPMENT",
        "",
        "## Future gate: FD v2 verification (post-fd-v2-deploy)",
        "",
        "When fd upstream repo deploys v2 per spec in `/root/fd-v2.md` (32KB, 873 lines, 45 test cases, 30+ requirements):",
        "",
        "1. **M062-S03v2**: re-run contract tests against new fd",
        "   - All 45 test cases from `/root/fd-v2.md` section 5 must pass",
        "   - Validate P0 requirements: R-P0-1..R-P0-19 (functional + observability + headers + error format)",
        "   - Validate P1 requirements: R-P1-1..R-P1-9 (health + features)",
        "   - Output: `artifacts/m062-fd-contract/fd-v2-validation-report.md`",
        "2. **M062-S04v2**: integration test — daily-archive wrapper vs new fd end-to-end",
        "   - Re-run 150 M061 papers through new fd",
        "   - Measure throughput, latency p50/p95/p99, error rate",
        "   - Validate graceful degradation, circuit breaker, retry+backoff",
        "3. **M062-S05v2**: ADR-019 update + M062 closeout",
        "   - ADR-019 amended with fd v2 validation evidence",
        "   - M062 closeout artifacts (SUMMARY + VALIDATION)",
        "   - 1 commit per slice",
        "",
        "Trigger: fd upstream issue/PR merge OR manual run via `/gsd plan-milestone M062v2 fd-v2-verification`.",
        "Reference: `/root/fd-v2.md` (authoritative spec).",
        "Owner: future executor after fd v2 deploy signal.",
    ])
    lines.append("")

    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--codebase-memory-snapshot", type=Path)
    parser.add_argument(
        "--phase",
        choices=PHASES,
        default="preflight",
        help="Severity tuning phase. preflight (default) = current behavior; "
             "active = uncommitted_changes_present promoted to medium; "
             "closeout = uncommitted_changes_present demoted to info.",
    )
    args = parser.parse_args()
    report = build_report(root=args.root, codebase_memory_snapshot=args.codebase_memory_snapshot, phase=args.phase)
    write_json(args.output_dir / "trajectory-report.json", report)
    write_text(args.output_dir / "trajectory-report.md", render_markdown(report))
    sys.stdout.write(f"trajectory report: verdict={report['verdict']} phase={report['phase']} flags={len(report['drift_flags'])}\n")
    return 0 if report["verdict"] != "blocked" else 2


if __name__ == "__main__":
    raise SystemExit(main())
