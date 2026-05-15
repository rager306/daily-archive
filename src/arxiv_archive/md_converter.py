"""PDF to Markdown converter using pymupdf."""

from pathlib import Path

import pymupdf


class MDConverter:
    """Converts PDF files to Markdown text."""

    def convert(self, pdf_path: Path) -> str:
        """Open PDF with pymupdf, extract text from all pages, join with double newline.

        Args:
            pdf_path: Path to the PDF file.

        Returns:
            Extracted text from all pages joined with double newlines.
        """
        doc = pymupdf.open(pdf_path)
        pages: list[str] = []
        for page in doc:
            text = str(page.get_text())
            pages.append(text if text is not None else "")
        doc.close()
        return "\n\n".join(pages)

    def convert_to_file(self, pdf_path: Path, output_path: Path) -> Path:
        """Convert PDF to markdown and write to output file.

        Args:
            pdf_path: Path to the source PDF file.
            output_path: Path to the output markdown file.

        Returns:
            The output_path that was written to.
        """
        content = self.convert(pdf_path)
        output_path.write_text(content, encoding="utf-8")
        return output_path
