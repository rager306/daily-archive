"""Public parser boundary for deterministic article structure."""

from arxiv_archive.parsing.normalization import slugify, strip_yaml_frontmatter
from arxiv_archive.parsing.parser import PARSER_VERSION, parse_article, with_parse_warning
from arxiv_archive.parsing.structure import ParsedArticle, ParsedArticleElement

__all__ = [
    "PARSER_VERSION",
    "ParsedArticle",
    "ParsedArticleElement",
    "parse_article",
    "slugify",
    "strip_yaml_frontmatter",
    "with_parse_warning",
]
