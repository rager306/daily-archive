//! audit-fields field extraction (ADR-045 / MEM508, M433).
//!
//! Extracts every `set_node_property_*(node, "field", …)` field name from a
//! function block. The scan operates on the **whole block text** from each
//! `set_node_property_` occurrence to the first quoted string — not
//! line-by-line — because `cargo fmt` routinely splits calls across lines
//! and the old line scan missed exactly those fields (M432: 2 of 4 drifted
//! fields were invisible to the audit).

use std::collections::HashSet;

/// Extract the set of field names from `set_node_property_*(…, "field", …)`
/// calls in `block`.
///
/// For each `set_node_property_` occurrence the scan locates the call's
/// opening `(`, walks to the matching `)`, and takes the first quoted
/// string **inside the argument list** (node ids are never string
/// literals). Calls whose field argument is a variable (dynamic fields,
/// e.g. `for (property, value) in […]`) contain no string literal and are
/// skipped — an unbounded forward scan would misattribute the next
/// unrelated string literal (M433: `create_edge(…, "hasSummary")` was
/// reported as a Summary property field).
pub fn extract_set_fields(block: &str) -> HashSet<String> {
    let bytes = block.as_bytes();
    let mut fields = HashSet::new();
    let mut cursor = 0usize;
    while let Some(rel) = block[cursor..].find("set_node_property_") {
        let start = cursor + rel;
        cursor = start + "set_node_property_".len();
        // Find the call's opening parenthesis.
        let Some(open_rel) = block[cursor..].find('(') else {
            continue;
        };
        let open = cursor + open_rel;
        // Walk to the matching closing parenthesis.
        let mut depth = 1usize;
        let mut i = open + 1;
        while i < bytes.len() && depth > 0 {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        let close = i.saturating_sub(1);
        if close <= open {
            continue; // unbalanced; skip rather than guess
        }
        // First quoted string inside the argument list.
        let args = &block[open + 1..close];
        if let Some(q1) = args.find('"') {
            let rest = &args[q1 + 1..];
            if let Some(q2) = rest.find('"') {
                fields.insert(rest[..q2].to_string());
            }
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_call_extracts() {
        let block = r#"
            store.set_node_property_string(node_id, "vid", vid_str.clone()).await?;
        "#;
        let fields = extract_set_fields(block);
        assert!(fields.contains("vid"), "{fields:?}");
        assert_eq!(fields.len(), 1);
    }

    /// M432 regression: cargo fmt splits the call across lines and the old
    /// line scan found no field at all.
    #[test]
    fn multi_line_call_extracts() {
        let block = r#"
            self.graph_store
                .set_node_property_string(
                    node_id,
                    "pdf_type",
                    format!("{:?}", classification.pdf_type),
                )
                .await?;
        "#;
        let fields = extract_set_fields(block);
        assert!(fields.contains("pdf_type"), "{fields:?}");
        assert_eq!(fields.len(), 1);
    }

    #[test]
    fn chained_calls_extract_both() {
        let block = r#"
            store.set_node_property_string(n, "title", t).await?;
            store.set_node_property_bool(
                n,
                "retrieval_eligible",
                true,
            )
            .await?;
        "#;
        let fields = extract_set_fields(block);
        assert!(
            fields.contains("title") && fields.contains("retrieval_eligible"),
            "{fields:?}"
        );
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn occurrence_without_string_is_ignored() {
        // A stray textual mention with no call after it yields nothing.
        let block = "let marker = \"set_node_property_\"; fn use_marker() {}";
        let fields = extract_set_fields(block);
        assert!(fields.is_empty(), "{fields:?}");
    }

    /// M433 false-positive regression: a dynamic-field call (the property is
    /// a loop variable) has no string literal in its argument list. An
    /// unbounded forward scan misattributed the NEXT string literal —
    /// `create_edge(…, "hasSummary")` — as a Summary property field.
    #[test]
    fn dynamic_field_call_does_not_leak_next_string_literal() {
        let block = r#"
            let summary = store.create_node("Summary").await.unwrap();
            for (property, value) in [("vid", "s-1"), ("text", "t")] {
                store
                    .set_node_property_string(summary, property, value)
                    .await
                    .unwrap();
            }
            store
                .create_edge(section, summary, "hasSummary")
                .await
                .unwrap();
        "#;
        let fields = extract_set_fields(block);
        assert!(
            !fields.contains("hasSummary"),
            "edge type must not leak in as a property field: {fields:?}"
        );
        assert!(
            fields.is_empty(),
            "dynamic call yields no field: {fields:?}"
        );
    }
}
