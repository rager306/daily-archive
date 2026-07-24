#!/usr/bin/env python3
"""Wave A operator: hybrid expand preflight + optional limited batch.

1. Plan next selection via hybrid_selection_expand inventory scan
2. Write proposal JSON (m213 selection shape)
3. Preflight PDF presence + already-bodied
4. Optional: --limit N live hybrid batch (default 0 = no batch)

Never authorizes import. Exit 0 after report generation.

Usage::

    uv run python scripts/verify_hybrid_expand_batch.py
    uv run python scripts/verify_hybrid_expand_batch.py --write artifacts/m213-hybrid-gate/selection-40-proposal.json
    uv run python scripts/verify_hybrid_expand_batch.py --limit 1 --enable-live-hybrid
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

from research_graph.application.corpus.hybrid_expand_preflight import (
    ProposedPaperCheck,
    preflight_hybrid_expand,
)
from research_graph.application.corpus.hybrid_expand_batch_gate import (
    evaluate_expand_batch_gate,
)
from research_graph.application.corpus.hybrid_selection_expand import (
    DEFAULT_MAX_BYTES,
    DEFAULT_TARGET_COUNT,
    InventoryPdfRow,
    plan_next_hybrid_selection,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS
from research_graph.workflows.composition.hybrid_batch_gate import (
    HybridBatchGateRequest,
    run_hybrid_batch_gate,
)
from research_graph.infrastructure.corpus.parsing.sidecar_services import (
    probe_parser_sidecars,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SELECTION = Path("artifacts/m213-hybrid-gate/selection-20.json")
DEFAULT_CATALOG = Path("data/article_catalog/article_catalog")
DEFAULT_PROPOSAL = Path("artifacts/m213-hybrid-gate/selection-40-proposal.json")
DEFAULT_BATCH_WORK = Path("artifacts/m213-hybrid-gate/runs-live-expand")


def _sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def scan_arxiv_pdf_inventory(catalog_root: Path, *, repo_root: Path) -> list[InventoryPdfRow]:
    rows: list[InventoryPdfRow] = []
    arxiv = catalog_root / "arxiv"
    if not arxiv.is_dir():
        return rows
    for pdf in sorted(arxiv.rglob("*.pdf")):
        if not pdf.is_file():
            continue
        try:
            rel = pdf.relative_to(arxiv)
        except ValueError:
            continue
        parts = rel.parts
        if len(parts) < 4:
            continue
        category, paper_id = parts[0], parts[1]
        try:
            rel_repo = pdf.resolve().relative_to(repo_root.resolve())
            pdf_path = str(rel_repo).replace("\\", "/")
        except ValueError:
            pdf_path = str(pdf)
        rows.append(
            InventoryPdfRow(
                paper_id=paper_id,
                category=category,
                pdf_path=pdf_path,
                byte_size=int(pdf.stat().st_size),
                sha256=_sha256_file(pdf),
            )
        )
    return rows


def load_selection_paper_ids(path: Path) -> set[str]:
    if not path.is_file():
        return set()
    data = json.loads(path.read_text(encoding="utf-8"))
    papers = data.get("papers") if isinstance(data, dict) else None
    if not isinstance(papers, list):
        return set()
    out: set[str] = set()
    for row in papers:
        if isinstance(row, dict):
            pid = str(row.get("paper_id") or "").strip()
            if pid:
                out.add(pid)
    return out


def discover_hybrid_body_ids(body_roots: list[Path]) -> set[str]:
    ids: set[str] = set()
    for root in body_roots:
        if not root.is_dir():
            continue
        for path in root.rglob("*.hybrid.body.md"):
            name = path.name
            if name.endswith(".hybrid.body.md"):
                pid = name[: -len(".hybrid.body.md")]
                if pid:
                    ids.add(pid)
    return ids


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave A hybrid expand: write proposal, preflight, optional limited batch. "
            "Import always false. Default --limit 0 (no batch)."
        )
    )
    parser.add_argument("--catalog-root", type=Path, default=DEFAULT_CATALOG)
    parser.add_argument("--selection", type=Path, default=DEFAULT_SELECTION)
    parser.add_argument(
        "--body-root",
        action="append",
        type=Path,
        default=None,
        help="Hybrid body root (repeatable)",
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--target-count", type=int, default=DEFAULT_TARGET_COUNT)
    parser.add_argument("--max-bytes", type=int, default=DEFAULT_MAX_BYTES)
    parser.add_argument("--rung", type=int, default=40)
    parser.add_argument("--milestone-id", type=str, default="M246-1xbli9")
    parser.add_argument(
        "--write",
        type=Path,
        default=DEFAULT_PROPOSAL,
        help="Path to write selection proposal JSON",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=0,
        help="Max papers for live batch (0 = preflight only)",
    )
    parser.add_argument(
        "--enable-live-hybrid",
        action="store_true",
        help="Enable live hybrid ports for batch (requires --limit > 0)",
    )
    parser.add_argument(
        "--min-hybrid-success",
        type=int,
        default=0,
        help="min_hybrid_success for batch gate_pass",
    )
    parser.add_argument(
        "--work-dir",
        type=Path,
        default=DEFAULT_BATCH_WORK,
        help="Work dir for limited live batch runs",
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument(
        "--refresh-continuity-pack",
        action=argparse.BooleanOptionalAction,
        default=True,
        help=(
            "After preflight/batch, refresh artifacts/etl/continuity-pack.json "
            "with live pack (incl. expand_gate). Default: on. "
            "Use --no-refresh-continuity-pack to skip. Never import."
        ),
    )
    parser.add_argument(
        "--continuity-pack-output",
        type=Path,
        default=Path("artifacts/etl/continuity-pack.json"),
        help="Path for --refresh-continuity-pack write",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _resolve(p: Path) -> Path:
        path = Path(p)
        return path if path.is_absolute() else (repo / path).resolve()

    catalog_root = _resolve(args.catalog_root)
    selection_path = _resolve(args.selection)
    write_path = _resolve(args.write)
    raw_roots = tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    body_roots = [_resolve(r) for r in raw_roots]
    work_dir = _resolve(args.work_dir)

    inventory = scan_arxiv_pdf_inventory(catalog_root, repo_root=repo)
    selected = load_selection_paper_ids(selection_path)
    bodied = discover_hybrid_body_ids(body_roots)
    exclude = frozenset(selected | bodied)

    expand = plan_next_hybrid_selection(
        inventory=inventory,
        exclude_paper_ids=exclude,
        target_count=args.target_count,
        max_bytes=args.max_bytes,
        rung=args.rung,
        extends=str(args.selection),
        milestone_id=args.milestone_id,
    )
    selection_payload = expand.to_selection_dict()
    write_path.parent.mkdir(parents=True, exist_ok=True)
    write_path.write_text(
        json.dumps(selection_payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    checks: list[ProposedPaperCheck] = []
    for paper in expand.proposed_papers:
        pdf = Path(paper.pdf_path)
        if not pdf.is_file():
            pdf = repo / paper.pdf_path
        checks.append(
            ProposedPaperCheck(
                paper_id=paper.paper_id,
                pdf_path=paper.pdf_path,
                pdf_exists=pdf.is_file(),
                already_bodied=paper.paper_id in bodied,
                byte_size=paper.byte_size,
            )
        )
    preflight = preflight_hybrid_expand(
        checks=checks,
        selection_path=str(write_path),
        target_count=args.target_count,
    )


    sidecars = probe_parser_sidecars(ensure=False)
    grobid_ok = bool(getattr(sidecars.grobid, "available", False))
    odl_ok = bool(getattr(sidecars.opendataloader, "available", False))
    batch_gate = evaluate_expand_batch_gate(
        preflight_signal=preflight.preflight_signal,
        ready_count=preflight.ready_count,
        limit=int(args.limit),
        enable_live_hybrid=bool(args.enable_live_hybrid),
        grobid_available=grobid_ok,
        odl_available=odl_ok,
        max_limit=20,
    )

    batch_summary: dict | None = None
    if batch_gate.allow_live_batch and int(args.limit) > 0 and args.enable_live_hybrid:
        ready_set = set(preflight.ready_paper_ids)
        limited_papers = [
            p
            for p in selection_payload["papers"]
            if isinstance(p, dict) and str(p.get("paper_id") or "") in ready_set
        ][: batch_gate.effective_limit]
        limited_path = write_path.with_name(
            write_path.stem
            + f"-limit{batch_gate.effective_limit}"
            + write_path.suffix
        )
        limited_sel = {
            **selection_payload,
            "count": len(limited_papers),
            "papers": limited_papers,
            "note": (
                "limited live batch subset; expand batch gate allowed; import false"
            ),
            "import_eligible": False,
            "graph_writes_allowed": False,
        }
        limited_path.write_text(
            json.dumps(limited_sel, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        result = run_hybrid_batch_gate(
            HybridBatchGateRequest(
                selection_path=limited_path,
                work_dir=work_dir,
                enable_live_hybrid=True,
                ensure_hybrid_containers=True,
                repo_root=repo,
                write_artifacts=True,
                min_hybrid_success=args.min_hybrid_success,
            )
        )
        batch_summary = {
            "selection_path": str(limited_path),
            "paper_count": result.paper_count,
            "hybrid_success_count": result.hybrid_success_count,
            "hybrid_deferred_count": result.hybrid_deferred_count,
            "error_count": result.error_count,
            "gate_pass": result.gate_pass,
            "expand_batch_gate": batch_gate.to_dict(),
            "import_eligible_any": False,
            "graph_writes_any": False,
        }
    elif int(args.limit) > 0:
        batch_summary = {
            "skipped": True,
            "reason": "expand_batch_gate_blocked",
            "expand_batch_gate": batch_gate.to_dict(),
            "import_eligible_any": False,
        }

    continuity_pack_summary: dict | None = None
    if args.refresh_continuity_pack:
        from research_graph.application.corpus.etl_continuity_pack import (
            compose_live_continuity_pack,
        )

        pack = compose_live_continuity_pack(
            repo_root=repo,
            body_roots=tuple(body_roots),
            expand_gate=batch_gate.to_dict(),
        )
        pack_path = _resolve(args.continuity_pack_output)
        pack_path.parent.mkdir(parents=True, exist_ok=True)
        pack_payload = pack.to_dict()
        pack_payload["import_eligible"] = False
        pack_payload["graph_writes_allowed"] = False
        pack_path.write_text(
            json.dumps(pack_payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        continuity_pack_summary = {
            "path": str(pack_path),
            "dashboard": dict(pack.dashboard),
            "alerts": list(pack.alerts),
            "closeout_signal": pack.closeout.closeout_signal,
            "import_eligible": False,
        }

    payload = {
        "expand_batch_gate": batch_gate.to_dict(),
        "continuity_pack": continuity_pack_summary,
        "schema_version": "m246-hybrid-expand-batch-report.v2",
        "proposal_path": str(write_path),
        "expand": {
            "proposed_count": expand.proposed_count,
            "available_after_filters": expand.available_after_filters,
            "inventory_count": expand.inventory_count,
        },
        "preflight": preflight.to_dict(),
        "batch": batch_summary,
        "import_eligible": False,
        "graph_writes_allowed": False,
        "note": "Wave A expand preflight/batch operator; never import",
    }

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = _resolve(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        batch_s = "none"
        if batch_summary:
            if batch_summary.get("skipped"):
                batch_s = f"skipped:{batch_summary.get('reason')}"
            else:
                batch_s = (
                    f"papers={batch_summary.get('paper_count')} "
                    f"hybrid_ok={batch_summary.get('hybrid_success_count')} "
                    f"errors={batch_summary.get('error_count')} "
                    f"gate_pass={batch_summary.get('gate_pass')}"
                )
        sys.stdout.write(
            "hybrid-expand-batch | "
            f"proposal: {write_path.name} | "
            f"proposed: {expand.proposed_count} | "
            f"preflight: {preflight.preflight_signal} | "
            f"ready: {preflight.ready_count} | "
            f"missing_pdf: {preflight.missing_pdf_count} | "
            f"already_bodied: {preflight.already_bodied_count} | "
            f"expand_gate: {batch_gate.gate_signal} | "
            f"eff_limit: {batch_gate.effective_limit} | "
            f"grobid: {batch_gate.grobid_available} | "
            f"odl: {batch_gate.odl_available} | "
            f"batch: {batch_s} | "
            f"limit: {args.limit} | "
            "import_eligible: false\n"
        )
        if batch_gate.reasons:
            sys.stdout.write(
                "  expand_gate_reasons: " + ", ".join(batch_gate.reasons) + "\n"
            )
        if continuity_pack_summary is not None:
            d = continuity_pack_summary.get("dashboard") or {}
            sys.stdout.write(
                "  continuity_pack: "
                f"hybrid_found={d.get('hybrid_found')} "
                f"closeout={continuity_pack_summary.get('closeout_signal')} "
                f"path={Path(continuity_pack_summary['path']).name}\n"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
