#!/usr/bin/env python3
"""Wave B constrained select operator (header / oracle / constrained LLM).

Usage::

    uv run python scripts/verify_wave_b_constrained_select.py
    uv run python scripts/verify_wave_b_constrained_select.py --mode oracle
    uv run python scripts/verify_wave_b_constrained_select.py --mode llm --live-llm
    uv run python scripts/verify_wave_b_constrained_select.py --json \\
        --output artifacts/wave-b/constrained-header-select.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from research_graph.application.corpus.wave_b_constrained_select import (
    header_priority_select,
    make_header_fallback_select_fn,
    make_llm_constrained_select_fn,
)
from research_graph.application.corpus.wave_b_extraction_baseline import (
    DEFAULT_HUMAN_GO_STAMP,
)
from research_graph.application.corpus.wave_b_gate import (
    evaluate_wave_b_gate,
    evaluate_wave_b_gate_from_stamp,
)
from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    score_gold_hybrid_constrained_pilot,
)
from research_graph.application.corpus.wave_b_gold_hybrid_join import (
    inventory_reviewed_gold_hybrid_join,
)
from research_graph.application.corpus.wave_b_gold_hybrid_lexical_metrics import (
    score_gold_hybrid_lexical_recovery,
)
from research_graph.application.reviewed_extraction_fixtures import (
    load_reviewed_extraction_split,
)
from research_graph.infrastructure.llm.ninerouter_client import NineRouterChatClient
from research_graph.infrastructure.llm.ninerouter_json_extract import (
    AGNES_25_PILOT_MODEL,
    AGNES_FREE_25_PILOT_MODEL,
    DEFAULT_PILOT_MODEL,
    QUALITY_PILOT_MODEL,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]


def _load_cases(repo: Path, *, gate_open: bool, human_go: bool, case_limit: int = 0):
    roots = tuple(
        (r if Path(r).is_absolute() else (repo / r).resolve()) for r in DEFAULT_BODY_ROOTS
    )
    train_g, _ = load_reviewed_extraction_split("train")
    val_g, _ = load_reviewed_extraction_split("validation")
    gold_all = train_g + val_g
    gold_by = {str(r.get("case_id")): r for r in gold_all if r.get("case_id")}
    join = inventory_reviewed_gold_hybrid_join(gold_records=gold_all, body_roots=roots)
    cases: list[dict] = []
    if gate_open and human_go:
        for row in join.joined:
            case_id = str(row.get("case_id") or "")
            gold = gold_by.get(case_id)
            if gold is None:
                continue
            path = Path(str(row.get("body_path") or ""))
            try:
                body = path.read_text(encoding="utf-8") if path.is_file() else ""
            except OSError:
                body = ""
            cases.append(
                {
                    "case_id": case_id,
                    "paper_id": str(row.get("paper_id") or ""),
                    "gold": gold,
                    "body_text": body,
                }
            )
        if case_limit and case_limit > 0:
            cases = cases[:case_limit]
    return cases, join


def _ninerouter_chat_fn(client: NineRouterChatClient):
    """Adapt NineRouterChatClient.chat -> make_llm_constrained_select_fn chat_fn."""

    def chat_fn(
        messages,
        *,
        model: str,
        max_tokens: int = 700,
        temperature: float = 0.0,
    ) -> str:
        result = client.chat(
            model=model,
            messages=messages,
            max_tokens=max_tokens,
            temperature=temperature,
        )
        if not result.ok:
            raise RuntimeError(result.error or "ninerouter chat failed")
        return str(result.text or result.content or "")

    return chat_fn


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B constrained select: header_priority (default), lexical oracle, "
            "or constrained LLM select (candidate_id only). Never free invent. "
            "Never DSPy/import."
        )
    )
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--no-stamp", action="store_true")
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument(
        "--mode",
        choices=("header", "oracle", "llm"),
        default="header",
        help=(
            "header=deterministic title select; oracle=gold coverage ceiling; "
            "llm=constrained candidate_id select via 9router (requires --live-llm)"
        ),
    )
    parser.add_argument("--max-body-chars", type=int, default=8000)
    parser.add_argument(
        "--case-limit",
        type=int,
        default=0,
        help="Optional max cases (0 = all joined)",
    )
    parser.add_argument(
        "--model",
        default=DEFAULT_PILOT_MODEL,
        help=(
            f"9router model for --mode llm (default {DEFAULT_PILOT_MODEL}; "
            f"agnes2.5={AGNES_25_PILOT_MODEL}; "
            f"agnes2.5-free={AGNES_FREE_25_PILOT_MODEL}; "
            f"quality={QUALITY_PILOT_MODEL})"
        ),
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=700,
        help="Chat max_tokens for --mode llm",
    )
    parser.add_argument(
        "--live-llm",
        action="store_true",
        help="Actually call 9router for --mode llm (default: dry, no network)",
    )
    parser.add_argument(
        "--llm-fallback-header",
        action="store_true",
        default=True,
        help="When --mode llm returns zero entities, fall back to header_priority (default on)",
    )
    parser.add_argument(
        "--no-llm-fallback-header",
        action="store_true",
        help="Disable header fallback for empty LLM selections",
    )
    parser.add_argument(
        "--compare-header",
        action="store_true",
        help="Also score header_priority baseline side-by-side (llm/oracle modes)",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)
    if args.no_stamp:
        gate = evaluate_wave_b_gate(human_go=False)
    else:
        raw = args.stamp if args.stamp is not None else DEFAULT_HUMAN_GO_STAMP
        stamp_path = raw if raw.is_absolute() else (repo / raw).resolve()
        gate = evaluate_wave_b_gate_from_stamp(stamp_path)

    cases, join = _load_cases(
        repo,
        gate_open=gate.wave_b_gate_open,
        human_go=gate.human_go,
        case_limit=int(args.case_limit),
    )
    floor_pkg = score_gold_hybrid_lexical_recovery(cases=cases)
    floor_metrics = dict(floor_pkg.metrics)

    if not (gate.wave_b_gate_open and gate.human_go):
        payload: dict[str, Any] = {
            "schema_version": "wave-b-constrained-select.v1",
            "operator_status": "blocked_gate",
            "mode": args.mode,
            "select_mode": args.mode,
            "case_count": 0,
            "metrics": None,
            "floor_metrics": floor_metrics,
            "import_eligible": False,
            "dspy_optimizer_enabled": False,
            "llm_used": False,
            "wave_b_gate_open": gate.wave_b_gate_open,
            "human_go": gate.human_go,
            "joined_count": join.joined_count,
        }
    elif args.mode == "llm" and not args.live_llm:
        # Process debt closeout: llm path exists but stays dry without explicit flag.
        header_pilot = score_gold_hybrid_constrained_pilot(
            cases=cases,
            select_fn=header_priority_select,
            floor_metrics=floor_metrics,
            max_body_chars=int(args.max_body_chars),
            llm_used=False,
            model_id="header_priority_select",
        )
        payload = header_pilot.to_dict()
        payload["operator_status"] = "llm_requires_live_flag"
        payload["select_mode"] = "llm"
        payload["llm_used"] = False
        payload["model_id"] = str(args.model)
        payload["note"] = (
            "Pass --live-llm to call 9router constrained select. "
            "Header baseline metrics included without network."
        )
        payload["header_baseline_metrics"] = dict(header_pilot.metrics)
        payload["wave_b_gate_open"] = gate.wave_b_gate_open
        payload["human_go"] = gate.human_go
        payload["joined_count"] = join.joined_count
        payload["scored_case_count"] = header_pilot.case_count
        payload["floor_entity_f1"] = floor_metrics.get("entity_f1")
        payload["floor_relation_f1"] = floor_metrics.get("relation_f1")
        payload["import_eligible"] = False
        payload["dspy_optimizer_enabled"] = False
    else:
        header_metrics: dict[str, Any] | None = None
        if args.compare_header or args.mode == "llm":
            header_pilot = score_gold_hybrid_constrained_pilot(
                cases=cases,
                select_fn=header_priority_select,
                floor_metrics=floor_metrics,
                max_body_chars=int(args.max_body_chars),
                llm_used=False,
                model_id="header_priority_select",
            )
            header_metrics = dict(header_pilot.metrics)

        ninerouter_diagnostics: dict[str, Any] | None = None
        if args.mode == "oracle":
            pilot = score_gold_hybrid_constrained_pilot(
                cases=cases,
                use_lexical_oracle=True,
                floor_metrics=floor_metrics,
                max_body_chars=int(args.max_body_chars),
                llm_used=False,
            )
            mode_name = "lexical_oracle_diagnostic"
            llm_used = False
            model_id = "lexical_oracle"
        elif args.mode == "llm":
            client = NineRouterChatClient()
            select_fn = make_llm_constrained_select_fn(
                chat_fn=_ninerouter_chat_fn(client),
                model=str(args.model),
                max_tokens=int(args.max_tokens),
                temperature=0.0,
            )
            use_fallback = bool(args.llm_fallback_header) and not bool(
                args.no_llm_fallback_header
            )
            if use_fallback:
                select_fn = make_header_fallback_select_fn(select_fn)
            pilot = score_gold_hybrid_constrained_pilot(
                cases=cases,
                select_fn=select_fn,
                floor_metrics=floor_metrics,
                max_body_chars=int(args.max_body_chars),
                llm_used=True,
                model_id=str(args.model),
            )
            mode_name = (
                "llm_constrained_select_with_header_fallback"
                if use_fallback
                else "llm_constrained_select"
            )
            llm_used = True
            model_id = str(args.model)
            ninerouter_diagnostics = dict(client.last_diagnostics)
        else:
            pilot = score_gold_hybrid_constrained_pilot(
                cases=cases,
                select_fn=header_priority_select,
                floor_metrics=floor_metrics,
                max_body_chars=int(args.max_body_chars),
                llm_used=False,
                model_id="header_priority_select",
            )
            mode_name = "header_priority_select"
            llm_used = False
            model_id = "header_priority_select"

        payload = pilot.to_dict()
        if ninerouter_diagnostics is not None:
            payload["ninerouter_diagnostics"] = ninerouter_diagnostics
        if args.mode == "llm":
            payload["llm_fallback_header"] = bool(args.llm_fallback_header) and not bool(
                args.no_llm_fallback_header
            )
        payload["operator_status"] = mode_name
        payload["select_mode"] = args.mode
        payload["llm_used"] = llm_used
        payload["model_id"] = model_id
        payload["wave_b_gate_open"] = gate.wave_b_gate_open
        payload["human_go"] = gate.human_go
        payload["joined_count"] = join.joined_count
        payload["scored_case_count"] = pilot.case_count
        payload["floor_entity_f1"] = floor_metrics.get("entity_f1")
        payload["floor_relation_f1"] = floor_metrics.get("relation_f1")
        payload["import_eligible"] = False
        payload["dspy_optimizer_enabled"] = False
        if header_metrics is not None:
            payload["header_baseline_metrics"] = header_metrics
            m = payload.get("metrics") if isinstance(payload.get("metrics"), dict) else {}
            payload["delta_vs_header"] = {
                "entity_f1": (m.get("entity_f1") or 0.0)
                - (header_metrics.get("entity_f1") or 0.0),
                "relation_f1": (m.get("relation_f1") or 0.0)
                - (header_metrics.get("relation_f1") or 0.0),
            }

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")
    if args.json:
        sys.stdout.write(text)
    else:
        m = payload.get("metrics") if isinstance(payload.get("metrics"), dict) else {}
        fm = (
            payload.get("floor_metrics")
            if isinstance(payload.get("floor_metrics"), dict)
            else {}
        )
        hb = (
            payload.get("header_baseline_metrics")
            if isinstance(payload.get("header_baseline_metrics"), dict)
            else {}
        )
        delta = (
            payload.get("delta_vs_header")
            if isinstance(payload.get("delta_vs_header"), dict)
            else {}
        )
        sys.stdout.write(
            "wave-b-constrained-select | "
            f"status: {payload.get('operator_status')} | "
            f"mode: {payload.get('select_mode') or args.mode} | "
            f"joined: {payload.get('joined_count')} | "
            f"scored: {payload.get('scored_case_count') or payload.get('case_count')} | "
            f"entity_f1: {m.get('entity_f1')} | "
            f"relation_f1: {m.get('relation_f1')} | "
            f"floor_entity_f1: {fm.get('entity_f1')} | "
            f"floor_relation_f1: {fm.get('relation_f1')} | "
            f"header_entity_f1: {hb.get('entity_f1')} | "
            f"delta_entity_f1: {delta.get('entity_f1')} | "
            f"llm: {str(payload.get('llm_used')).lower()} | "
            f"model: {payload.get('model_id') or 'none'} | "
            "dspy: false | import_eligible: false\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
