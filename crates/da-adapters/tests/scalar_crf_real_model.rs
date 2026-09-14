//! Optional smoke harness for an audited local GROBID CRF model resource.
//!
//! No model bytes are committed or discovered implicitly. The test is an
//! explicit missingness report unless the caller supplies a local path.

use da_adapters::FilesystemCrfModelLoader;
use da_ports::validate_loaded_crf_model;

const MODEL_PATH_ENV: &str = "DA_GROBID_CRF_MODEL_PATH";

#[test]
fn optional_real_model_resource_is_explicitly_missing_or_validated() {
    let Some(path) = std::env::var_os(MODEL_PATH_ENV).filter(|value| !value.is_empty()) else {
        eprintln!(
            "MISSING: {MODEL_PATH_ENV} is not configured; no real GROBID model claim was made"
        );
        return;
    };

    let loader = FilesystemCrfModelLoader::new(path);
    let model = validate_loaded_crf_model(&loader)
        .unwrap_or_else(|error| panic!("configured real CRF model failed validation: {error}"));

    assert_eq!(model.model_type, 2, "resource must be a scalar CRF model");
    assert!(!model.labels.is_empty(), "resource must declare labels");
    eprintln!(
        "PASS: validated configured local scalar CRF resource (labels={}, observations={}, nftr={})",
        model.labels.len(),
        model.observations.len(),
        model.nftr
    );
}
