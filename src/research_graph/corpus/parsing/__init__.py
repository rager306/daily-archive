"""Public parser boundary for deterministic article structure."""

from research_graph.corpus.parsing.normalization import slugify, strip_yaml_frontmatter
from research_graph.corpus.parsing.parser import PARSER_VERSION, parse_article, with_parse_warning
from research_graph.corpus.parsing.structure import ParsedArticle, ParsedArticleElement

__all__ = [
    "PARSER_VERSION",
    "ParsedArticle",
    "ParsedArticleElement",
    "parse_article",
    "slugify",
    "strip_yaml_frontmatter",
    "with_parse_warning",
]
