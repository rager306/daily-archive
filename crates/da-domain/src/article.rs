//! Article structure types (GRAPH-SCHEMA.md article spine).
//!
//! Section, Keyword, Topic, Category — the "обвязка статьи" that wraps
//! the paper before extracted entities.

use crate::schema::{FieldType, NodeSchemaDef};
use serde::{Deserialize, Serialize};

// ─── Section ───

/// A structural section of a paper (from GROBID TEI `<div><head>`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub vid: crate::vid::Vid,
    pub title: String,
    pub level: u32,
    pub order: u32,
    pub text: String,
    pub paper_id: String,
}

/// Schema definition for Section nodes.
pub struct SectionSchema;

impl NodeSchemaDef for SectionSchema {
    fn label(&self) -> &'static str {
        "Section"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("title", FieldType::String),
            ("level", FieldType::Integer),
            ("order", FieldType::Integer),
            ("paper_id", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("text", FieldType::String),
            ("char_count", FieldType::Integer),
        ]
    }
}

// ─── Keyword ───

/// A YAKE-extracted keyword.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyword {
    pub vid: crate::vid::Vid,
    pub keyword: String,
    pub score: f64,
    pub language: String,
    pub paper_id: String,
}

/// Schema definition for Keyword nodes.
pub struct KeywordSchema;

impl NodeSchemaDef for KeywordSchema {
    fn label(&self) -> &'static str {
        "Keyword"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("keyword", FieldType::String),
            ("score", FieldType::Float),
            ("paper_id", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("language", FieldType::String)]
    }
}

// ─── Topic ───

/// A research topic/theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub vid: crate::vid::Vid,
    pub label: String,
    pub source: String, // category / keyword / title
    pub confidence: f32,
}

/// Schema definition for Topic nodes.
pub struct TopicSchema;

impl NodeSchemaDef for TopicSchema {
    fn label(&self) -> &'static str {
        "Topic"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("label", FieldType::String),
            ("source", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("confidence", FieldType::Float)]
    }
}

// ─── Category ───

/// An arXiv category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub vid: crate::vid::Vid,
    pub code: String,
    pub name: String,
    pub is_primary: bool,
}

/// Schema definition for Category nodes.
pub struct CategorySchema;

impl NodeSchemaDef for CategorySchema {
    fn label(&self) -> &'static str {
        "Category"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("code", FieldType::String),
            ("is_primary", FieldType::Boolean),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("name", FieldType::String)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_schema() {
        let s = SectionSchema;
        assert_eq!(s.label(), "Section");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "paper_id"));
    }

    #[test]
    fn test_keyword_schema() {
        let s = KeywordSchema;
        assert_eq!(s.label(), "Keyword");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "score"));
    }

    #[test]
    fn test_topic_schema() {
        let s = TopicSchema;
        assert_eq!(s.label(), "Topic");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "source"));
    }

    #[test]
    fn test_category_schema() {
        let s = CategorySchema;
        assert_eq!(s.label(), "Category");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "code"));
    }
}
