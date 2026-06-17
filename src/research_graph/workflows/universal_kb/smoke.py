# Formerly: src/arxiv_archive/universal_kb_smoke.py

"""Unified command surface for Universal KB no-write smoke runs."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from scripts.audit_m036_real_corpus_smoke import audit_smoke, write_json, write_markdown_report
from scripts.run_m036_real_corpus_no_write_smoke import run_smoke
from scripts.select_m036_real_corpus_smoke_batch import select_entries

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_BASE_DIR = ROOT / "artifacts" / "m036-real-corpus-no-write-smoke"
PROFILES = frozenset({"fast", "full"})
MIN_SMOKE_ARTICLES = 3
MAX_SMOKE_ARTICLES = 30
FORBIDDEN_PAYLOAD_TERMS = (
    "api_key",
    "secret_value",
    "bearer ",
    "x-api-key",
    "embedding_payload",
    "vector_payload",
    "chunk_text_payload",
    "paper_text_payload",
    "claim_text_payload",
)


@dataclass(frozen=True, slots=True)
class SmokePaths:
    base_dir: Path = DEFAULT_BASE_DIR

    @property
    def manifest(self) -> Path:
        return self.base_dir / "manifest.json"

    @property
    def run_dir(self) -> Path:
        return self.base_dir / "run"

    @property
    def run_summary(self) -> Path:
        return self.run_dir / "summary.json"

    @property
    def audit_json(self) -> Path:
        return self.base_dir / "audit.json"

    @property
    def audit_md(self) -> Path:
        return self.base_dir / "audit.md"


def emit(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def require_profile(profile: str) -> None:
    if profile not in PROFILES:
        raise ValueError(f"profile must be one of {sorted(PROFILES)}")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_manifest(path: Path, *, limit: int) -> dict[str, Any]:
    if limit < MIN_SMOKE_ARTICLES or limit > MAX_SMOKE_ARTICLES:
        raise ValueError(f"limit must be between {MIN_SMOKE_ARTICLES} and {MAX_SMOKE_ARTICLES}")
    entries = select_entries(limit)
    if len(entries) < 3:
        raise ValueError(f"only selected {len(entries)} usable articles; need at least 3")
    payload = {
        "schema_version": "m036-real-corpus-smoke-manifest.v1",
        "catalog_ref": "artifact:data/article_catalog/catalog.json",
        "article_count": len(entries),
        "articles": entries,
        "safety_flags": {
            "graph_write_allowed": False,
            "promotion_allowed": False,
            "production_import_attempted": False,
            "import_eligible": False,
        },
        "diagnostics": sorted({diagnostic for entry in entries for diagnostic in entry["diagnostics"]}),
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return payload


def resolve_paths(paths: SmokePaths | None) -> SmokePaths:
    return paths if paths is not None else SmokePaths()


def run_select(*, limit: int, paths: SmokePaths | None = None) -> dict[str, Any]:
    resolved_paths = resolve_paths(paths)
    manifest = write_manifest(resolved_paths.manifest, limit=limit)
    return summarize(paths=resolved_paths, profile="fast", manifest=manifest)


def run_runner(*, paths: SmokePaths | None = None, clean: bool = True) -> dict[str, Any]:
    resolved_paths = resolve_paths(paths)
    return run_smoke(resolved_paths.manifest, output_dir=resolved_paths.run_dir, clean=clean)


def run_audit(*, paths: SmokePaths | None = None) -> dict[str, Any]:
    resolved_paths = resolve_paths(paths)
    audit = audit_smoke(resolved_paths.manifest, resolved_paths.run_dir)
    write_json(resolved_paths.audit_json, audit)
    write_markdown_report(audit, resolved_paths.audit_md)
    return audit


def scan_json_artifacts(base_dir: Path) -> int:
    files = [path for path in base_dir.rglob("*.json") if path.is_file()]
    for path in files:
        text = path.read_text(encoding="utf-8", errors="ignore").lower()
        for term in FORBIDDEN_PAYLOAD_TERMS:
            if term in text:
                raise ValueError(f"{path} contains forbidden payload term {term!r}")
    return len(files)


SAFETY_KEYS = ("graph_write_allowed", "promotion_allowed", "production_import_attempted", "import_eligible")


def require_false_flags(payload: dict[str, Any], *, label: str) -> None:
    for key in SAFETY_KEYS:
        if payload.get(key) is not False:
            raise ValueError(f"{label}.{key} must be false")


def require_article_flags_false(articles: Any, *, label: str) -> None:
    if not isinstance(articles, list):
        raise ValueError(f"{label} must be a list")
    for index, article in enumerate(articles):
        if not isinstance(article, dict):
            raise ValueError(f"{label}[{index}] must be an object")
        if "safety_flags" in article and isinstance(article["safety_flags"], dict):
            require_false_flags(article["safety_flags"], label=f"{label}[{index}].safety_flags")
        else:
            require_false_flags(article, label=f"{label}[{index}]")


def validate_persisted_state(manifest_payload: dict[str, Any], run_summary: dict[str, Any], audit: dict[str, Any]) -> None:
    require_false_flags(manifest_payload.get("safety_flags", {}), label="manifest.safety_flags")
    require_article_flags_false(manifest_payload.get("articles", []), label="manifest.articles")
    if run_summary:
        require_false_flags(run_summary, label="summary")
        require_article_flags_false(run_summary.get("articles", []), label="summary.articles")
    if audit:
        require_false_flags(audit.get("safety", {}), label="audit.safety")


def summarize(*, paths: SmokePaths, profile: str, manifest: dict[str, Any] | None = None) -> dict[str, Any]:
    manifest_payload = manifest if manifest is not None else load_json(paths.manifest)
    run_summary = load_json(paths.run_summary) if paths.run_summary.exists() else {}
    audit = load_json(paths.audit_json) if paths.audit_json.exists() else {}
    validate_persisted_state(manifest_payload, run_summary, audit)
    return {
        "profile": profile,
        "article_count": int(run_summary.get("article_count") or manifest_payload.get("article_count") or 0),
        "completed_handoff_count": int(run_summary.get("completed_handoff_count") or 0),
        "blockers_for_import": audit.get("blockers_for_import", []),
        "json_artifacts_scanned": scan_json_artifacts(paths.base_dir) if paths.base_dir.exists() else 0,
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def run_m035_verifier() -> None:
    subprocess.run([sys.executable, "scripts/verify_m035_universal_kb_prototype.py"], cwd=ROOT, check=True)


def run_all(*, limit: int = 5, profile: str = "fast", paths: SmokePaths | None = None) -> dict[str, Any]:
    require_profile(profile)
    resolved_paths = resolve_paths(paths)
    if profile == "full":
        run_m035_verifier()
    run_select(limit=limit, paths=resolved_paths)
    run_runner(paths=resolved_paths, clean=True)
    run_audit(paths=resolved_paths)
    return summarize(paths=resolved_paths, profile=profile)


def run_verify(*, profile: str = "fast", paths: SmokePaths | None = None) -> dict[str, Any]:
    require_profile(profile)
    resolved_paths = resolve_paths(paths)
    if profile == "full":
        run_m035_verifier()
    if not resolved_paths.manifest.exists() or not resolved_paths.run_summary.exists() or not resolved_paths.audit_json.exists():
        raise FileNotFoundError("smoke manifest, run summary, and audit artifacts must exist before verify")
    result = summarize(paths=resolved_paths, profile=profile)
    if result["article_count"] < MIN_SMOKE_ARTICLES or result["article_count"] > MAX_SMOKE_ARTICLES:
        raise AssertionError(
            f"smoke verification must cover between {MIN_SMOKE_ARTICLES} and {MAX_SMOKE_ARTICLES} articles"
        )
    if result["completed_handoff_count"] != result["article_count"]:
        raise AssertionError("smoke must complete all selected handoffs")
    return result


def print_result(result: dict[str, Any]) -> None:
    emit(f"profile={result['profile']}")
    emit(f"articles={result['article_count']}")
    emit(f"handoffs={result['completed_handoff_count']}")
    emit(f"blockers={','.join(result['blockers_for_import']) or 'none'}")
    emit("graph_write_allowed=false promotion_allowed=false production_import_attempted=false import_eligible=false")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Universal KB no-write smoke command surface")
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command in ("select", "run", "audit", "verify", "all"):
        sub = subparsers.add_parser(command)
        sub.add_argument("--base-dir", type=Path, default=DEFAULT_BASE_DIR)
        sub.add_argument("--profile", choices=sorted(PROFILES), default="fast")
        if command in {"select", "all"}:
            sub.add_argument("--limit", type=int, default=5)
        if command == "run":
            sub.add_argument("--clean", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    paths = SmokePaths(base_dir=args.base_dir)
    if args.command == "select":
        result = run_select(limit=args.limit, paths=paths)
    elif args.command == "run":
        run_runner(paths=paths, clean=args.clean)
        result = summarize(paths=paths, profile=args.profile)
    elif args.command == "audit":
        run_audit(paths=paths)
        result = summarize(paths=paths, profile=args.profile)
    elif args.command == "verify":
        result = run_verify(profile=args.profile, paths=paths)
    elif args.command == "all":
        result = run_all(limit=args.limit, profile=args.profile, paths=paths)
    else:  # pragma: no cover
        raise AssertionError(args.command)
    print_result(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
