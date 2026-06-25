#!/usr/bin/env python3
"""Inventory repo-local Python write paths for architecture review.

This is a deliberately small static scanner. It finds obvious file/database write
operations and applies conservative categories; it is not a data-flow engine.
Unknowns are expected and should be reviewed by humans.
"""

from __future__ import annotations

import argparse
import ast
import json
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

ROOTS = (Path("src/research_graph"), Path("scripts"))
WRITE_ATTRS = {"write_text", "write_bytes"}
OPEN_ATTRS = {"open"}
DB_CONNECT_MODULES = {"sqlite3"}
SCHEMA_VERSION = "daily-archive-write-path-inventory.v1"


@dataclass(frozen=True)
class WritePathRecord:
    path: str
    line: int
    operation: str
    target: str
    category: str
    reason: str


def _unparse(node: ast.AST | None) -> str:
    if node is None:
        return "<unknown>"
    try:
        return ast.unparse(node)
    except Exception:  # pragma: no cover - defensive for unusual AST nodes
        return "<unknown>"


def _literal_text(node: ast.AST | None) -> str | None:
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        return node.value
    return None


def _mode_from_call(node: ast.Call) -> str | None:
    if len(node.args) >= 2:
        literal = _literal_text(node.args[1])
        if literal is not None:
            return literal
    for keyword in node.keywords:
        if keyword.arg == "mode":
            literal = _literal_text(keyword.value)
            if literal is not None:
                return literal
    return None


def _call_name(func: ast.AST) -> str:
    if isinstance(func, ast.Attribute):
        return f"{_unparse(func.value)}.{func.attr}"
    return _unparse(func)


def _classify(source_path: Path, operation: str, target: str, mode: str | None) -> tuple[str, str]:
    path_text = f"{source_path} {target}".lower()
    if source_path.parts and source_path.parts[0] == "scripts":
        return "script-only", "write occurs in process-boundary script"
    if operation == "sqlite3.connect" or ".db" in path_text or "database" in path_text:
        return "database", "database-backed mutable state"
    if mode and "a" in mode:
        return "append-log", "append mode"
    if any(token in path_text for token in ("events", "jsonl", "diagnostics")):
        return "append-log", "event or diagnostics log path"
    if any(token in path_text for token in ("queue", "state", "index", "catalog")):
        return "shared-state", "stable shared state or index path"
    if any(token in path_text for token in ("output", "artifact", "run", "day_dir", "summary")):
        return "run-scoped", "caller/output scoped artifact path"
    return "unknown", "static scanner could not infer ownership"


class WritePathVisitor(ast.NodeVisitor):
    def __init__(self, path: Path) -> None:
        self.path = path
        self.records: list[WritePathRecord] = []

    def visit_Call(self, node: ast.Call) -> Any:
        operation: str | None = None
        target: str | None = None
        mode: str | None = None

        if isinstance(node.func, ast.Attribute) and node.func.attr in WRITE_ATTRS:
            operation = node.func.attr
            target = _unparse(node.func.value)
        elif isinstance(node.func, ast.Attribute) and node.func.attr in OPEN_ATTRS:
            mode = _mode_from_call(node)
            if mode and any(flag in mode for flag in ("w", "a", "+")):
                operation = "Path.open"
                target = _unparse(node.func.value)
        elif isinstance(node.func, ast.Name) and node.func.id == "open":
            mode = _mode_from_call(node)
            if mode and any(flag in mode for flag in ("w", "a", "+")):
                operation = "open"
                target = _unparse(node.args[0] if node.args else None)
        elif isinstance(node.func, ast.Attribute):
            call_name = _call_name(node.func)
            if call_name in {"sqlite3.connect"}:
                operation = call_name
                target = _unparse(node.args[0] if node.args else None)

        if operation and target:
            category, reason = _classify(self.path, operation, target, mode)
            self.records.append(
                WritePathRecord(
                    path=str(self.path),
                    line=node.lineno,
                    operation=operation,
                    target=target,
                    category=category,
                    reason=reason,
                )
            )
        self.generic_visit(node)


def collect_records(roots: tuple[Path, ...] = ROOTS) -> list[WritePathRecord]:
    records: list[WritePathRecord] = []
    for root in roots:
        if not root.exists():
            continue
        for path in sorted(root.rglob("*.py")):
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            visitor = WritePathVisitor(path)
            visitor.visit(tree)
            records.extend(visitor.records)
    return records


def build_payload(records: list[WritePathRecord]) -> dict[str, Any]:
    by_category = Counter(record.category for record in records)
    by_root = Counter(record.path.split("/", 1)[0] for record in records)
    return {
        "schema_version": SCHEMA_VERSION,
        "summary": {
            "total_records": len(records),
            "by_category": dict(sorted(by_category.items())),
            "by_root": dict(sorted(by_root.items())),
        },
        "records": [asdict(record) for record in records],
    }


def render_markdown(payload: dict[str, Any]) -> str:
    lines = [
        "# M167 Write Path Inventory",
        "",
        f"Schema: `{payload['schema_version']}`",
        "",
        "## Summary",
        "",
        f"Total records: `{payload['summary']['total_records']}`",
        "",
        "| Category | Count |",
        "|---|---:|",
    ]
    for category, count in payload["summary"]["by_category"].items():
        lines.append(f"| {category} | {count} |")
    lines.extend(["", "## Records", "", "| Path | Line | Operation | Target | Category |", "|---|---:|---|---|---|"])
    for record in payload["records"]:
        target = str(record["target"]).replace("|", "\\|")
        lines.append(
            f"| `{record['path']}` | {record['line']} | `{record['operation']}` | "
            f"`{target}` | {record['category']} |"
        )
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", type=Path, required=True, help="Path for JSON inventory output")
    parser.add_argument("--markdown", type=Path, required=True, help="Path for Markdown inventory output")
    args = parser.parse_args()

    records = collect_records()
    payload = build_payload(records)
    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown.write_text(render_markdown(payload), encoding="utf-8")
    print(json.dumps(payload["summary"], sort_keys=True))


if __name__ == "__main__":
    main()
