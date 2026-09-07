//! Deterministic, redacted P2 → native body-accounting shadow receipt.
//! This example is intentionally not a production CLI command or ParserPort path.

use da_parse::{LayoutDocument, NativeBodySequenceInput, run_native_body_sequence};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = LayoutDocument::new("shadow-example", "a".repeat(64), vec![])?;
    let receipt = run_native_body_sequence(NativeBodySequenceInput {
        document: &document,
        segmentation: None,
        p2_status: "terminal_failure",
        p2_failure_code: Some("example_requires_source_bound_p2_receipt"),
    });
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
