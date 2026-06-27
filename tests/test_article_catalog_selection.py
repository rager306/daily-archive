from __future__ import annotations

import json
from pathlib import Path

import scripts.verify_article_catalog as verify_article_catalog
from research_graph.application.corpus.article_catalog_selection import (
    build_current_catalog_index_selection,
)


def test_build_current_catalog_index_selection_filters_invalid_rows(tmp_path: Path) -> None:
    index_path = tmp_path / "index.json"
    index_path.write_text(
        json.dumps(
            {
                "articles": [
                    {"article_ref": "artifact:a/article.json", "source_code": "arxiv", "title": "A"},
                    {"article_ref": "artifact:missing-source/article.json", "title": "skip"},
                    {"source_code": "arxiv", "title": "skip"},
                    "skip",
                ]
            }
        ),
        encoding="utf-8",
    )

    selection = build_current_catalog_index_selection(index_path)

    assert selection["schema_version"] == "article-corpus-selection.v00.01"
    assert selection["selection_id"] == "current-article-catalog-index"
    assert selection["network_policy"] == {
        "test_phase_must_not_fetch": True,
        "pipeline_phase_reads_catalog_only": True,
    }
    assert selection["articles"] == [
        {"article_ref": "artifact:a/article.json", "source_code": "arxiv", "title": "A"}
    ]


def test_verify_article_catalog_no_arg_wrapper_builds_temp_selection(
    monkeypatch, tmp_path: Path
) -> None:
    index_path = tmp_path / "index.json"
    catalog_path = tmp_path / "catalog.json"
    index_path.write_text(
        json.dumps(
            {
                "articles": [
                    {
                        "article_ref": "artifact:catalog/article.json",
                        "source_code": "local",
                        "title": "Local article",
                    }
                ]
            }
        ),
        encoding="utf-8",
    )
    catalog_path.write_text("{}", encoding="utf-8")
    captured: dict[str, object] = {}

    def fake_run_core(argv: list[str]) -> int:
        captured["argv"] = argv
        selection_path = Path(argv[argv.index("--selection") + 1])
        captured["selection"] = json.loads(selection_path.read_text(encoding="utf-8"))
        return 0

    monkeypatch.setattr(verify_article_catalog, "DEFAULT_INDEX", index_path)
    monkeypatch.setattr(verify_article_catalog, "DEFAULT_CATALOG", catalog_path)
    monkeypatch.setattr(verify_article_catalog, "run_core", fake_run_core)

    assert verify_article_catalog.run(["verify_article_catalog.py"]) == 0
    captured_selection = captured["selection"]
    captured_argv = captured["argv"]
    assert isinstance(captured_selection, dict)
    assert isinstance(captured_argv, list)
    assert captured_selection == build_current_catalog_index_selection(index_path)
    assert captured_argv == [
        "verify_article_catalog.py",
        "--catalog",
        str(catalog_path),
        "--index",
        str(index_path),
        "--selection",
        str(Path(captured_argv[6])),
        "--validate-only",
        "--require-index",
        "--check-index-titles",
    ]


def test_verify_article_catalog_explicit_args_delegate_unchanged(monkeypatch) -> None:
    explicit_argv = ["verify_article_catalog.py", "--catalog", "catalog.json"]
    captured: dict[str, list[str]] = {}

    def fake_run_core(argv: list[str]) -> int:
        captured["argv"] = argv
        return 7

    monkeypatch.setattr(verify_article_catalog, "run_core", fake_run_core)

    assert verify_article_catalog.run(explicit_argv) == 7
    assert captured["argv"] is explicit_argv
