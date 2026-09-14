use da_domain::scalar_crf::{
    CrfModel, CrfModelError, MAX_CRF_MODEL_BYTES, decode_viterbi, map_input,
};
use da_ports::{CrfModelLoadError, CrfModelLoaderPort, validate_loaded_crf_model};

pub const SYNTHETIC_FIXTURE: &str = "scalar_crf_synthetic.wapiti";
pub const VARIANT_FIXTURE: &str = "scalar_crf_synthetic_variant.wapiti";
pub const MALFORMED_FIXTURE: &str = "scalar_crf_malformed.wapiti";

pub fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

pub struct FakeCrfLoader {
    pub result: Result<CrfModel, CrfModelLoadError>,
}

impl CrfModelLoaderPort for FakeCrfLoader {
    fn load(&self) -> Result<CrfModel, CrfModelLoadError> {
        self.result.clone()
    }
}

/// Shared behavioral contract for a resource loader, independent of its I/O mechanism.
/// Expected values are hand-computed from the fixture: Y=2, uoff=[0,2], boff=[4], nftr=8.
pub fn assert_loader_contract(loader: &dyn CrfModelLoaderPort) {
    let model = validate_loaded_crf_model(loader).expect("synthetic CRF loader should validate");
    assert_eq!(model.model_type, 2);
    assert_eq!(model.labels, ["A", "B"]);
    assert_eq!(model.observations, ["u00:cat", "u00:dog", "b"]);
    assert_eq!(model.u_offsets, [Some(0), Some(2), None]);
    assert_eq!(model.b_offsets, [None, None, Some(4)]);
    assert_eq!(model.nftr, 8);
    assert_eq!(model.weights.get(&0), Some(&1.0));
    assert_eq!(model.weights.get(&3), Some(&3.0));
    assert_eq!(model.weights.get(&4), Some(&2.0));
    assert_eq!(model.weights.get(&5), Some(&2.0));
    assert!(
        !model.weights.contains_key(&1),
        "sparse zero must stay absent"
    );

    let positions = map_input(&model, "cat\ndog").expect("fixture input should map");
    assert_eq!(positions.len(), 2);
    assert_eq!(positions[0].uobs, [0]);
    assert_eq!(positions[0].bobs, [2]);
    assert_eq!(positions[1].uobs, [1]);
    assert_eq!(positions[1].bobs, [2]);

    // Independent worked oracle: position 0=A (1), position 1=B (1+3+2=6).
    let decoded = decode_viterbi(&model, &positions).expect("fixture should decode");
    assert_eq!(decoded.label_ids, [0, 1]);
    assert_eq!(decoded.labels, ["A", "B"]);
    assert_eq!(decoded.scores, [1.0, 6.0]);
}

pub fn assert_malformed_model(bytes: &[u8]) {
    assert!(matches!(
        CrfModel::parse(bytes),
        Err(CrfModelError::IndexOutOfRange { index: 8, nftr: 8 })
    ));
}

pub fn assert_oversize_error(error: CrfModelLoadError) {
    assert!(matches!(
        error,
        CrfModelLoadError::ResourceTooLarge { actual, maximum }
            if actual == MAX_CRF_MODEL_BYTES as u64 + 1
                && maximum == MAX_CRF_MODEL_BYTES as u64
    ));
}
