use da_parse::{
    FeatureDisposition, NATIVE_FEATURE_COLUMNS, NATIVE_FEATURE_SCHEMA_VERSION, NativeFeatureOutput,
    StructuralFeatureRow,
};
use proptest::prelude::*;

fn row(index: u32, disposition: FeatureDisposition) -> StructuralFeatureRow {
    StructuralFeatureRow {
        page_ordinal: index / 32,
        source_ordinal: index,
        token_ordinal: index,
        token: format!("sha256:{index:064x}"),
        line_status: "line_boundary".into(),
        block_status: "body".into(),
        font: None,
        font_size: None,
        relative_position: format!("0:0:{index}"),
        capitalisation: disposition.clone(),
        digitisation: disposition.clone(),
        punctuation: disposition.clone(),
        lexicon_flags: disposition,
        label: None,
    }
}

proptest! {
    #[test]
    fn ordered_columns_are_unique_and_byte_stable(_seed in any::<u64>()) {
        let joined = NATIVE_FEATURE_COLUMNS.join("\t");
        prop_assert_eq!(NATIVE_FEATURE_COLUMNS.len(), 11);
        prop_assert_eq!(NATIVE_FEATURE_COLUMNS.iter().collect::<std::collections::BTreeSet<_>>().len(), 11);
        prop_assert_eq!(joined, "token\tline_status\tblock_status\tfont\tfont_size\trelative_position\tcapitalisation\tdigitisation\tpunctuation\tlexicon_flags\tlabel");
    }

    #[test]
    fn structural_rows_roundtrip_without_payload_or_drift(count in 0usize..128) {
        let rows = (0..count as u32)
            .map(|i| row(i, FeatureDisposition::UnsupportedLexiconBacked))
            .collect::<Vec<_>>();
        let output = NativeFeatureOutput {
            schema_version: NATIVE_FEATURE_SCHEMA_VERSION.into(),
            document_id: "fixture".into(),
            lattice_fingerprint: "sha256:layout".into(),
            admitted: count,
            conserved: count,
            loss: 0,
            disposition: FeatureDisposition::StructuralPass,
            rows,
        };
        let bytes = serde_json::to_vec(&output).unwrap();
        let replay: NativeFeatureOutput = serde_json::from_slice(&bytes).unwrap();
        prop_assert_eq!(&output, &replay);
        prop_assert_eq!(&serde_json::to_vec(&replay).unwrap(), &bytes);
        prop_assert!(output.rows.len() <= output.admitted);
        prop_assert_eq!(output.admitted, output.conserved);
        prop_assert_eq!(output.loss, 0);
        prop_assert!(!String::from_utf8(bytes).unwrap().contains("fixture text"));
    }

    #[test]
    fn terminal_dispositions_never_become_structural_pass(code in "[a-z_]{1,32}") {
        let disposition = FeatureDisposition::PropagatedTerminal { code: code.clone() };
        let encoded = serde_json::to_string(&disposition).unwrap();
        let decoded: FeatureDisposition = serde_json::from_str(&encoded).unwrap();
        prop_assert_eq!(&decoded, &disposition);
        prop_assert_ne!(&decoded, &FeatureDisposition::StructuralPass);
        prop_assert!(encoded.contains(&code));
    }
}
