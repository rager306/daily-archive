"""Tests for md_converter module."""

from pathlib import Path

import pymupdf
import pytest

from arxiv_archive.md_converter import MDConverter


def test_md_converter_init():
    """Test that MDConverter can be instantiated and has a convert method."""
    converter = MDConverter()
    assert hasattr(converter, "convert")
    assert callable(converter.convert)


def test_convert_returns_string(tmp_path):
    """Test that convert extracts text from a minimal PDF and returns a string."""
    # Create a minimal PDF using pymupdf
    pdf_path = tmp_path / "test.pdf"
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((50, 50), "Hello, World!", fontsize=12)
    doc.save(str(pdf_path))
    doc.close()

    converter = MDConverter()
    result = converter.convert(pdf_path)

    assert isinstance(result, str)
    assert "Hello, World!" in result


def test_convert_to_file(tmp_path):
    """Test that convert_to_file writes markdown content to a file."""
    pdf_path = tmp_path / "test.pdf"
    output_path = tmp_path / "output.md"

    # Create a minimal PDF
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((50, 50), "Test content", fontsize=12)
    doc.save(str(pdf_path))
    doc.close()

    converter = MDConverter()
    returned_path = converter.convert_to_file(pdf_path, output_path)

    assert returned_path == output_path
    assert output_path.exists()
    content = output_path.read_text(encoding="utf-8")
    assert "Test content" in content
