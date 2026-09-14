use da_domain::{
    FeatureTemplate, GROBID_LEXICON_SCHEMA_VERSION, GROBID_RESOURCE_VERSION, LexiconBundle,
    LexiconChannel,
};
use da_ports::{LexiconLoadError, LexiconLoaderPort, validate_loaded_lexicon};

pub fn valid_bundle() -> LexiconBundle {
    LexiconBundle {
        schema_version: GROBID_LEXICON_SCHEMA_VERSION.into(),
        resource_version: GROBID_RESOURCE_VERSION.into(),
        channels: vec![LexiconChannel {
            key: "chemical".into(),
            entries: vec!["acid".into(), "water".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "layout-basic".into(),
            channel_keys: vec!["chemical".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

pub struct FakeLexiconLoader {
    pub result: Result<LexiconBundle, LexiconLoadError>,
}

impl LexiconLoaderPort for FakeLexiconLoader {
    fn load(&self) -> Result<LexiconBundle, LexiconLoadError> {
        self.result.clone()
    }
}

pub fn assert_loader_contract(loader: &dyn LexiconLoaderPort) {
    let bundle = validate_loaded_lexicon(loader).expect("loader returns valid lexicon");
    assert_eq!(bundle, valid_bundle());
}
