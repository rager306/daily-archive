#!/usr/bin/env python3
"""Probe Adaptix mapping for M033 OpenDataLoader PDF JSON outputs.

This is a bounded research adapter, not a production importer. It loads the
fixed OpenDataLoader JSON schema into typed intermediate models and writes
review-only candidate summaries for S05/S06 synthesis.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from adaptix import Retort, name_mapping
from adaptix.struct_trail import get_trail

SAFETY_FLAGS = {
    "graph_import_allowed": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "trusted_kg_import_allowed": False,
    "import_eligible": False,
}


@dataclass(frozen=True)
class OdlElement:
    """Typed view of a common OpenDataLoader element.

    OpenDataLoader elements are heterogeneous. Fields not modeled here are kept
    in ``extra`` so the adapter can inspect schema drift without dropping data.
    """

    type: str
    page_number: int | None = None
    bounding_box: tuple[float, float, float, float] | None = None
    id: int | None = None
    content: str | None = None
    extra: Mapping[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class OdlDocument:
    file_name: str
    number_of_pages: int
    author: str | None = None
    title: str | None = None
    kids: list[OdlElement] = field(default_factory=list)
    extra: Mapping[str, Any] = field(default_factory=dict)


RETORT = Retort(
    recipe=[
        name_mapping(
            OdlDocument,
            map={
                "file_name": "file name",
                "number_of_pages": "number of pages",
            },
            extra_in="extra",
        ),
        name_mapping(
            OdlElement,
            map={
                "page_number": "page number",
                "bounding_box": "bounding box",
            },
            extra_in="extra",
        ),
    ]
)


def read_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise TypeError(f"Expected JSON object at {path}")
    return data


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def append_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")


def flatten_dicts(value: Any) -> Iterable[dict[str, Any]]:
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from flatten_dicts(child)
    elif isinstance(value, list):
        for child in value:
            yield from flatten_dicts(child)


def load_odl_document(raw: Mapping[str, Any]) -> OdlDocument:
    return RETORT.load(raw, OdlDocument)


def paper_json_paths(probe_root: Path) -> list[Path]:
    paths = sorted(probe_root.glob("per-paper/*/hybrid/original.json"))
    if not paths:
        raise FileNotFoundError(f"No per-paper OpenDataLoader JSON outputs under {probe_root}")
    return paths


def article_key_from_path(path: Path) -> str:
    return path.parents[1].name


def summarize_document(article_key: str, source_path: Path, raw: dict[str, Any]) -> dict[str, Any]:
    doc = load_odl_document(raw)
    all_objects = list(flatten_dicts(raw))
    typed_type_counts = Counter(element.type for element in doc.kids)
    raw_type_counts = Counter(str(obj.get("type")) for obj in all_objects if obj.get("type"))
    raw_key_counts = Counter(key for obj in all_objects for key in obj)
    top_level_extra_counts = Counter(key for element in doc.kids for key in element.extra)
    page_numbers = sorted(
        {element.page_number for element in doc.kids if element.page_number is not None}
    )
    elements_with_bbox = sum(1 for element in doc.kids if element.bounding_box is not None)
    elements_with_content = sum(1 for element in doc.kids if element.content)
    heading_count = sum(count for typ, count in raw_type_counts.items() if "heading" in typ.lower())
    table_count = sum(count for typ, count in raw_type_counts.items() if "table" in typ.lower())
    figure_count = sum(
        count
        for typ, count in raw_type_counts.items()
        if any(marker in typ.lower() for marker in ("image", "figure", "picture", "caption"))
    )
    candidate_summary = {
        "source_ref_candidate": {
            "article_key": article_key,
            "source_output_path": str(source_path),
            "file_name": doc.file_name,
            "title": doc.title,
            "number_of_pages": doc.number_of_pages,
            "candidate_only": True,
        },
        "page_index_candidate": {
            "page_count": doc.number_of_pages,
            "pages_observed_in_elements": page_numbers,
            "top_level_element_count": len(doc.kids),
            "elements_with_bounding_boxes": elements_with_bbox,
        },
        "semantic_signal_candidate": {
            "heading_count": heading_count,
            "table_signal_count": table_count,
            "figure_caption_signal_count": figure_count,
            "elements_with_content": elements_with_content,
        },
        "adapter_boundary": {
            "adaptix_loaded": True,
            "typed_model": "OdlDocument",
            "extra_fields_preserved": True,
            "top_level_extra_keys": dict(top_level_extra_counts.most_common(20)),
            "raw_top_keys": dict(raw_key_counts.most_common(20)),
        },
    }
    return {
        "article_key": article_key,
        "status": "mapped_candidate_only",
        "source_path": str(source_path),
        "metrics": {
            "top_level_elements": len(doc.kids),
            "raw_object_count": len(all_objects),
            "typed_type_counts": dict(typed_type_counts.most_common(20)),
            "raw_type_counts": dict(raw_type_counts.most_common(30)),
            "raw_key_counts": dict(raw_key_counts.most_common(30)),
        },
        "candidate_summary": candidate_summary,
        "safety_flags": dict(SAFETY_FLAGS),
    }


def diagnostic(
    article_key: str, severity: str, code: str, message: str, path: str | None = None
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "article_key": article_key,
        "severity": severity,
        "code": code,
        "message": message,
        "safety_flags": dict(SAFETY_FLAGS),
    }
    if path is not None:
        payload["path"] = path
    return payload


def render_report(results: list[dict[str, Any]], diagnostics: list[dict[str, Any]]) -> str:
    lines = [
        "# M033 OpenDataLoader Adaptix Adapter Probe",
        "",
        "## Verdict",
        "",
        "`adaptix-adapter-candidate` if all rows mapped; otherwise `needs-attention`.",
        "",
        "This report is review-only. It does not claim graph readiness, production import eligibility, or LadybugDB write readiness.",
        "",
        "## Per-paper mapping",
        "",
        "| Article | Status | Top-level elements | Raw objects | Headings | Tables | Figures/Captions |",
        "|---|---|---:|---:|---:|---:|---:|",
    ]
    for result in results:
        signals = result["candidate_summary"]["semantic_signal_candidate"]
        metrics = result["metrics"]
        lines.append(
            "| {article} | `{status}` | {top} | {raw} | {headings} | {tables} | {figures} |".format(
                article=result["article_key"],
                status=result["status"],
                top=metrics["top_level_elements"],
                raw=metrics["raw_object_count"],
                headings=signals["heading_count"],
                tables=signals["table_signal_count"],
                figures=signals["figure_caption_signal_count"],
            )
        )
    lines += ["", "## Diagnostics", ""]
    if diagnostics:
        for diag in diagnostics:
            lines.append(
                f"- `{diag['severity']}` `{diag['code']}` {diag['article_key']}: {diag['message']}"
            )
    else:
        lines.append("No adapter diagnostics were emitted.")
    lines += [
        "",
        "## Safety",
        "",
        "- `graph_import_allowed=false`",
        "- `ladybugdb_written=false`",
        "- `production_import_attempted=false`",
        "- `import_eligible=false`",
    ]
    return "\n".join(lines) + "\n"


def run_probe(probe_root: Path, output_dir: Path) -> int:
    results: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    for path in paper_json_paths(probe_root):
        article_key = article_key_from_path(path)
        try:
            raw = read_json(path)
            results.append(summarize_document(article_key, path, raw))
            diagnostics.append(
                diagnostic(
                    article_key,
                    "info",
                    "adaptix_mapping_succeeded",
                    "OpenDataLoader JSON loaded into typed Adaptix model and candidate summary was generated.",
                    str(path),
                )
            )
        except Exception as exc:  # noqa: BLE001 - probe must convert failures to diagnostics
            trail = ""
            try:
                trail = "/" + "/".join(map(str, get_trail(exc)))
            except Exception:  # noqa: BLE001
                trail = ""
            diagnostics.append(
                diagnostic(
                    article_key,
                    "error",
                    "adaptix_mapping_failed",
                    f"{type(exc).__name__}: {exc}",
                    f"{path}{trail}",
                )
            )
    status = (
        "adaptix-adapter-candidate"
        if results and not any(d["severity"] == "error" for d in diagnostics)
        else "needs-attention"
    )
    summary = {
        "schema": "m033.opendataloader_adaptix_adapter.summary.v1",
        "status": status,
        "probe_root": str(probe_root),
        "paper_count": len(results),
        "diagnostic_count": len(diagnostics),
        "error_count": sum(1 for d in diagnostics if d["severity"] == "error"),
        "results": results,
        "safety_flags": dict(SAFETY_FLAGS),
    }
    write_json(output_dir / "adaptix-adapter-summary.json", summary)
    append_jsonl(output_dir / "adaptix-adapter-diagnostics.jsonl", diagnostics)
    (output_dir / "adaptix-adapter-report.md").write_text(
        render_report(results, diagnostics), encoding="utf-8"
    )
    sys.stdout.write(
        json.dumps(
            {"status": status, "paper_count": len(results), "error_count": summary["error_count"]},
            indent=2,
        )
        + "\n"
    )
    return 0 if status == "adaptix-adapter-candidate" else 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe-root", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    return run_probe(args.probe_root, args.output_dir)


if __name__ == "__main__":
    raise SystemExit(main())
