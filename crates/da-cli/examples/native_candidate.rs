//! Explicit offline PDF candidate. Never selects a production backend or writes a graph.
use clap::Parser;
use da_adapters::native_pdf_visual::extract_native_pdf_visual;
use da_adapters::{LayoutDonorLimits, PdfInspectorLayoutDonor};
use da_cli::visual_adapter::native_visual_observation;
use da_parse::native_candidate_emission::emit_native_candidate;
use da_parse::native_scholarly_candidate::{
    build_native_header_candidate, build_native_reference_candidate_with_headings,
};
use da_parse::{
    FullSemanticRegion, HeadingRegionConfig, NativeVisualConfig, TeiLifter, assign_heading_regions,
    build_native_word_region_carrier, detect_figure_heads, project_native_words,
};
use da_ports::{CrfModel, LexiconBundle};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Parser)]
struct Args {
    /// Trusted local PDF only. Native donor/lopdf parsing is not a memory or time sandbox.
    #[arg(long)]
    pdf: PathBuf,
    #[arg(long)]
    document_id: String,
    /// Caller-owned pinned model/renderer/lexicon association; no auto-discovery.
    #[arg(long)]
    model_manifest: PathBuf,
    /// New directory only; never overwrites a frozen receipt or an existing run.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    lexicon: Resource,
    header: Option<ModelResource>,
    reference_segmenter: Option<ModelResource>,
    citation: Option<ModelResource>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    path: PathBuf,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelResource {
    resource: Resource,
    feature_schema: String,
    feature_columns: usize,
    ordered_labels: Vec<String>,
    embedded_patterns_sha256: String,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_pinned(
    root: &Path,
    resource: &Resource,
    maximum: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let path = root.join(&resource.path);
    if fs::metadata(&path)?.len() > u64::try_from(maximum)? {
        return Err("pinned resource exceeds existing size limit".into());
    }
    let bytes = fs::read(&path)?;
    if bytes.len() > maximum || sha(&bytes) != resource.sha256 {
        return Err("pinned resource digest/size mismatch".into());
    }
    Ok(bytes)
}

fn load_model(
    root: &Path,
    spec: &ModelResource,
    schema: &str,
    columns: usize,
    admissible: &[&str],
) -> Result<CrfModel, Box<dyn Error>> {
    if spec.feature_schema != schema || spec.feature_columns != columns {
        return Err("model feature schema/column association mismatch".into());
    }
    let bytes = read_pinned(root, &spec.resource, da_ports::MAX_CRF_MODEL_BYTES)?;
    let model = CrfModel::parse(&bytes)?;
    if model.model_type != 2 || model.labels != spec.ordered_labels {
        return Err("scalar model type/ordered label association mismatch".into());
    }
    let allowed: BTreeSet<_> = admissible.iter().copied().collect();
    let labels: BTreeSet<_> = model.labels.iter().map(String::as_str).collect();
    if labels != allowed || model.labels.len() != allowed.len() {
        return Err("model label vocabulary differs from declared stage family".into());
    }
    let patterns: Vec<_> = model
        .patterns
        .iter()
        .map(|pattern| &pattern.source)
        .collect();
    if sha(&serde_json::to_vec(&patterns)?) != spec.embedded_patterns_sha256 {
        return Err("embedded model pattern association mismatch".into());
    }
    Ok(model)
}

fn header_summary(stage: &Result<da_parse::NativeHeaderCandidate, String>) -> serde_json::Value {
    match stage {
        Ok(value) => json!({"status": "executed_candidate", "feature_digest": value.feature_digest,
            "source_rows": value.rows.len(), "title_values": value.fields.title.len(),
            "email_values": value.fields.email.len(), "identifier_values": value.fields.ids.len(),
            "date_spans": value.dates.len(), "single_recognized_date": value.recognized_date().is_some(),
            "author_run_leaves": value.authors.len(), "affiliation_run_leaves": value.affiliations.len()}),
        Err(error) => json!({"status": "refused", "reason": error}),
    }
}

fn reference_summary(
    stage: &Result<da_parse::NativeReferenceCandidate, String>,
) -> serde_json::Value {
    match stage {
        Ok(value) => json!({"status": "executed_candidate",
            "reference_feature_digest": value.reference_feature_digest,
            "citation_feature_digest": value.citation_feature_digest,
            "records": value.records.len(), "physical_lines": value.lines.len(),
            "complete_within_one_labelled_physical_line_envelope": value.has_complete_labelled_line_inventory()}),
        Err(error) => json!({"status": "refused", "reason": error}),
    }
}

fn model_identity(spec: &Option<ModelResource>) -> serde_json::Value {
    match spec {
        Some(model) => json!({"model_sha256": model.resource.sha256,
            "feature_schema": model.feature_schema, "feature_columns": model.feature_columns,
            "ordered_labels": model.ordered_labels,
            "embedded_patterns_sha256": model.embedded_patterns_sha256}),
        None => serde_json::Value::Null,
    }
}

fn effective_configuration(manifest: &Manifest) -> Result<String, Box<dyn Error>> {
    Ok(sha(&serde_json::to_vec(&json!({
        "schema_version": manifest.schema_version,
        "lexicon_sha256": manifest.lexicon.sha256,
        "header": model_identity(&manifest.header),
        "reference_segmenter": model_identity(&manifest.reference_segmenter),
        "citation": model_identity(&manifest.citation),
    }))?))
}

fn load_header(root: &Path, manifest: &Manifest) -> Result<CrfModel, Box<dyn Error>> {
    let spec = manifest
        .header
        .as_ref()
        .ok_or("header model not configured")?;
    load_model(
        root,
        spec,
        da_parse::NATIVE_HEADER_FEATURE_SCHEMA_VERSION,
        da_parse::NATIVE_HEADER_FEATURE_COLUMNS,
        da_parse::NATIVE_HEADER_LABELS,
    )
}

fn load_references(
    root: &Path,
    manifest: &Manifest,
) -> Result<(CrfModel, CrfModel), Box<dyn Error>> {
    let reference = manifest
        .reference_segmenter
        .as_ref()
        .ok_or("reference-segmenter model not configured")?;
    let citation = manifest
        .citation
        .as_ref()
        .ok_or("citation model not configured")?;
    Ok((
        load_model(
            root,
            reference,
            da_parse::NATIVE_BIBLIOGRAPHY_FEATURES_SCHEMA_VERSION,
            da_parse::NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
            &da_parse::NATIVE_REFERENCE_SEGMENTER_LABELS,
        )?,
        load_model(
            root,
            citation,
            da_parse::NATIVE_BIBLIOGRAPHY_FEATURES_SCHEMA_VERSION,
            da_parse::NATIVE_CITATION_FEATURE_COLUMNS,
            &da_parse::NATIVE_CITATION_LABELS,
        )?,
    ))
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    if args.document_id.trim().is_empty() {
        return Err("document id is empty".into());
    }
    let manifest_bytes = fs::read(&args.model_manifest)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.schema_version != "da-cli.native-candidate-models/v1" {
        return Err("unknown candidate model manifest version".into());
    }
    let root = args
        .model_manifest
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let lexicon_bytes = read_pinned(
        root,
        &manifest.lexicon,
        usize::try_from(da_adapters::MAX_LEXICON_BYTES)?,
    )?;
    let lexicon: LexiconBundle = serde_yaml::from_slice(&lexicon_bytes)?;
    lexicon.validate()?;
    let header_model = load_header(root, &manifest);
    let reference_models = load_references(root, &manifest);
    let limits = LayoutDonorLimits::default();
    if fs::metadata(&args.pdf)?.len() > u64::try_from(limits.max_output_bytes)? {
        return Err("PDF input exceeds the existing donor admission byte limit".into());
    }
    let pdf = fs::read(&args.pdf)?;
    let source_sha = sha(&pdf);
    let observation =
        PdfInspectorLayoutDonor.extract_lexical_shadow(&pdf, &args.document_id, limits)?;
    let projection = project_native_words(&observation.document, &observation.lexemes)?;
    let heading = assign_heading_regions(
        &observation.document,
        &observation.lattice,
        &projection,
        HeadingRegionConfig::default(),
    )?;
    let carrier = build_native_word_region_carrier(
        &observation.document,
        &observation.lattice,
        &projection,
        &heading,
        &[
            FullSemanticRegion::Header,
            FullSemanticRegion::References,
            FullSemanticRegion::Body,
        ],
    )?;
    let body = carrier
        .run(FullSemanticRegion::Body)
        .map(|run| {
            run.tokens
                .iter()
                .map(|token| token.text())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let header = match &header_model {
        Ok(model) => build_native_header_candidate(&carrier, &lexicon, model)
            .map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    };
    let references = match &reference_models {
        Ok((reference, citation)) => build_native_reference_candidate_with_headings(
            &carrier, &lexicon, reference, citation, &heading,
        )
        .map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    };
    let config_sha = effective_configuration(&manifest)?;
    let emission = emit_native_candidate(
        header.as_ref().ok(),
        references.as_ref().ok(),
        &body,
        &source_sha,
        &config_sha,
    );
    let (emission_summary, lifted_summary) = match &emission {
        Ok(emitted) => {
            let lifted = TeiLifter::new().lift(&emitted.envelope, &args.document_id, &source_sha);
            (
                json!({"status": "emitted_candidate", "manifest": emitted.manifest}),
                json!({"status": "executed", "title": lifted.title, "citations": lifted.citations.len()}),
            )
        }
        Err(error) => (
            json!({"status": "refused", "reason": error.to_string()}),
            json!({"status": "not_run", "reason": "emission_refused"}),
        ),
    };
    let header_model_status = match &header_model {
        Ok(_) => json!({"status": "loaded"}),
        Err(error) => json!({"status": "refused", "reason": error.to_string()}),
    };
    let reference_models_status = match &reference_models {
        Ok(_) => json!({"status": "loaded"}),
        Err(error) => json!({"status": "refused", "reason": error.to_string()}),
    };
    let visual = match extract_native_pdf_visual(&pdf, &args.document_id, limits) {
        Ok(extraction) => match native_visual_observation(&extraction.document, &extraction.visual)
        {
            Ok(native) => match detect_figure_heads(&native, NativeVisualConfig::default()) {
                Ok(heads) => json!({"status": "candidate_observation", "source": extraction.visual,
                    "heads": heads, "content_digest_scope": "encoded_image_xobject_stream_bytes_not_pixels",
                    "semantic_figure_caption_link_acceptance": false}),
                Err(error) => json!({"status": "refused", "reason": error.to_string()}),
            },
            Err(reason) => json!({"status": "refused", "reason": reason}),
        },
        Err(error) => json!({"status": "refused", "reason": error.to_string()}),
    };
    let report = json!({
        "schema_version": "da-cli.native-candidate-run/v1",
        "source_pdf_sha256": source_sha,
        "model_manifest_sha256": sha(&manifest_bytes),
        "effective_configuration_sha256": config_sha,
        "lexicon_sha256": manifest.lexicon.sha256,
        "carrier": carrier,
        "header": header_summary(&header),
        "references": reference_summary(&references),
        "model_loading": {"header": header_model_status, "reference_segmenter_and_citation": reference_models_status},
        "emission": emission_summary,
        "visual": visual,
        "lifted_summary": lifted_summary,
        "claim_scope": "opt_in_stage_wiring_not_independent_model_parity_or_product_acceptance",
        "body_structure": "single_paragraph_from_native_body_run_not_fulltext_structure",
        "renderer_abi_upstream_equivalence": "not_proven",
        "backend_resource_savings": "not_measured",
        "resource_boundary": "trusted_offline_source_only_admission_checks_not_peak_memory_or_time_sandbox",
        "input_sources": "pdf_and_explicit_local_model_lexicon_resources_no_java_tei",
        "production_route_changed": false,
        "import_eligible": false, "deployed": false,
        "auto_promote": "forbidden", "graph_write": "forbidden"
    });
    write_candidate_outputs(&args.out, &report, emission)?;
    println!("candidate TEI and stage report: {}", args.out.display());
    Ok(())
}

fn write_candidate_outputs(
    out: &Path,
    report: &serde_json::Value,
    emission: Result<da_parse::NativeCandidateEmission, da_parse::NativeCandidateEmissionError>,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir(out)?;
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(report)?)?;
    match emission {
        Ok(emitted) => fs::write(out.join("candidate.tei.xml"), emitted.envelope.raw_tei())?,
        Err(error) => {
            eprintln!(
                "candidate emission refused; stage report: {}",
                out.join("report.json").display()
            );
            return Err(Box::new(error));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_bytes() -> Vec<u8> {
        b"#mdl#2#0\n#rdr#1/1/0\n9:u:%x[0,0],\n#qrk#1\n5:<one>,\n#qrk#0\n".to_vec()
    }

    fn spec(path: PathBuf, bytes: &[u8]) -> ModelResource {
        ModelResource {
            resource: Resource {
                path,
                sha256: sha(bytes),
            },
            feature_schema: "fixture/v1".into(),
            feature_columns: 1,
            ordered_labels: vec!["<one>".into()],
            embedded_patterns_sha256: sha(&serde_json::to_vec(&vec!["u:%x[0,0]"]).unwrap()),
        }
    }

    #[test]
    fn refused_emission_preserves_report_without_tei_and_returns_original_error() {
        let directory = tempfile::tempdir().unwrap();
        let out = directory.path().join("refusal");
        let report = json!({
            "header": {"source_rows": 3},
            "references": {"physical_lines": 80},
            "visual": {"image_count": 2},
            "emission": {"status": "refused"},
        });
        let error = write_candidate_outputs(
            &out,
            &report,
            Err(da_parse::NativeCandidateEmissionError::LimitsExceeded(
                "limitations",
            )),
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<da_parse::NativeCandidateEmissionError>(),
            Some(&da_parse::NativeCandidateEmissionError::LimitsExceeded(
                "limitations"
            ))
        );
        let retained: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("report.json")).unwrap()).unwrap();
        assert_eq!(retained, report);
        assert!(!out.join("candidate.tei.xml").exists());

        let replacement = json!({"replacement": true});
        assert!(
            write_candidate_outputs(
                &out,
                &replacement,
                Err(da_parse::NativeCandidateEmissionError::Identity)
            )
            .is_err()
        );
        let retained: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("report.json")).unwrap()).unwrap();
        assert_eq!(retained, report, "existing run must not be overwritten");
    }

    #[test]
    fn pins_bind_model_bytes_labels_patterns_and_renderer_before_inference() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fixture.wapiti");
        let bytes = model_bytes();
        fs::write(&path, &bytes).unwrap();
        let mut pinned = spec(path, &bytes);
        assert!(load_model(directory.path(), &pinned, "fixture/v1", 1, &["<one>"]).is_ok());
        pinned.feature_columns = 2;
        assert!(load_model(directory.path(), &pinned, "fixture/v1", 1, &["<one>"]).is_err());
        pinned.feature_columns = 1;
        pinned.ordered_labels = vec!["<other>".into()];
        assert!(load_model(directory.path(), &pinned, "fixture/v1", 1, &["<one>"]).is_err());
        pinned.ordered_labels = vec!["<one>".into()];
        pinned.embedded_patterns_sha256 = "0".repeat(64);
        assert!(load_model(directory.path(), &pinned, "fixture/v1", 1, &["<one>"]).is_err());
        pinned.embedded_patterns_sha256 = sha(&serde_json::to_vec(&vec!["u:%x[0,0]"]).unwrap());
        fs::write(&pinned.resource.path, b"substituted").unwrap();
        assert!(load_model(directory.path(), &pinned, "fixture/v1", 1, &["<one>"]).is_err());
    }

    #[test]
    fn effective_configuration_excludes_resource_paths_but_not_model_identity() {
        let mut manifest = Manifest {
            schema_version: "fixture/v1".into(),
            lexicon: Resource {
                path: "first.yaml".into(),
                sha256: "a".repeat(64),
            },
            header: Some(spec("first.wapiti".into(), &model_bytes())),
            reference_segmenter: None,
            citation: None,
        };
        let identity = effective_configuration(&manifest).unwrap();
        manifest.lexicon.path = "elsewhere.yaml".into();
        manifest.header.as_mut().unwrap().resource.path = "elsewhere.wapiti".into();
        assert_eq!(effective_configuration(&manifest).unwrap(), identity);
        manifest.header.as_mut().unwrap().resource.sha256 = "b".repeat(64);
        assert_ne!(effective_configuration(&manifest).unwrap(), identity);
    }
}

fn main() -> ExitCode {
    let _guard = da_cli::logging::init_pipeline_logging("native_candidate");
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("native_candidate: {error}");
            ExitCode::FAILURE
        }
    }
}
