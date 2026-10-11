//! Non-authoritative visual analysis candidate (M371 S03, ADR-053).
//! Stage-1 figure type classification (ADR-060) is candidate-only and DATA-driven.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

const BUNDLED_FIGURE_TYPE_LABELS: &str =
    include_str!("../../../data/ontology/figure_type_labels.yaml");
const BUNDLED_FIGURE_SCHEMAS: &str = include_str!("../../../data/ontology/figure_schemas.yaml");
const BUNDLED_FIGURE_PANEL_MARKERS: &str =
    include_str!("../../../data/ontology/figure_panel_markers.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualAnalysisType {
    Flowchart,
    Plot,
    Diagram,
    Table,
    Screenshot,
    Photograph,
    Other,
}

impl VisualAnalysisType {
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart",
            Self::Plot => "plot",
            Self::Diagram => "diagram",
            Self::Table => "table",
            Self::Screenshot => "screenshot",
            Self::Photograph => "photograph",
            Self::Other => "other",
        }
    }

    pub fn from_wire(wire: &str) -> Option<Self> {
        match wire.trim().to_ascii_lowercase().as_str() {
            "flowchart" => Some(Self::Flowchart),
            "plot" | "chart" => Some(Self::Plot),
            "diagram" => Some(Self::Diagram),
            "table" => Some(Self::Table),
            "screenshot" => Some(Self::Screenshot),
            "photograph" | "photo" => Some(Self::Photograph),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// How Stage-1 decided the type (heuristic is free; vlm is optional later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FigureClassifyMethod {
    Heuristic,
    Vlm,
}

/// Stage-1 output: typed route for Stage-2 schemas. Never graph truth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureTypeClassification {
    pub artifact_id: String,
    pub visual_type: VisualAnalysisType,
    pub confidence: f32,
    pub method: FigureClassifyMethod,
    pub matched_keywords: Vec<String>,
    pub subpanel_count: u32,
    /// Detected panel letters (a, b, c). Empty means single undivided panel.
    #[serde(default)]
    pub panel_labels: Vec<String>,
    pub labels_schema_version: String,
    pub context_sha256: String,
    pub image_sha256: String,
    pub authoritative: bool,
    pub import_eligible: bool,
}

impl FigureTypeClassification {
    pub fn validate(&self) -> Result<(), FigureAnalysisError> {
        if self.artifact_id.trim().is_empty() {
            return Err(FigureAnalysisError::Missing("artifact_id"));
        }
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(FigureAnalysisError::Invalid(
                "confidence must be finite and between 0 and 1".into(),
            ));
        }
        if self.subpanel_count == 0 {
            return Err(FigureAnalysisError::Invalid(
                "subpanel_count must be >= 1".into(),
            ));
        }
        if !self.panel_labels.is_empty() && self.panel_labels.len() != self.subpanel_count as usize
        {
            return Err(FigureAnalysisError::Invalid(
                "panel_labels length must match subpanel_count".into(),
            ));
        }
        if self.labels_schema_version.trim().is_empty() {
            return Err(FigureAnalysisError::Missing("labels_schema_version"));
        }
        for (field, value) in [
            ("context_sha256", self.context_sha256.as_str()),
            ("image_sha256", self.image_sha256.as_str()),
        ] {
            if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(FigureAnalysisError::Invalid(format!(
                    "{field} must be a SHA-256 digest"
                )));
            }
        }
        if self.authoritative || self.import_eligible {
            return Err(FigureAnalysisError::AuthorityViolation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FigureTypeLabelsFile {
    schema_version: String,
    default_type: String,
    types: Vec<FigureTypeLabelEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct FigureTypeLabelEntry {
    id: String,
    wire: String,
    weight: u32,
    caption_keywords: Vec<String>,
}

/// DATA-driven Stage-1 label table (bundled YAML or loaded string).
#[derive(Debug, Clone)]
pub struct FigureTypeLabelTable {
    pub schema_version: String,
    default_type: VisualAnalysisType,
    entries: Vec<(VisualAnalysisType, u32, Vec<String>)>,
}

impl FigureTypeLabelTable {
    pub fn bundled() -> Result<Self, FigureAnalysisError> {
        Self::from_yaml(BUNDLED_FIGURE_TYPE_LABELS)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, FigureAnalysisError> {
        let file: FigureTypeLabelsFile = serde_yaml::from_str(yaml)
            .map_err(|e| FigureAnalysisError::Invalid(format!("figure_type_labels.yaml: {e}")))?;
        if file.schema_version != "figure-type-labels.v1" {
            return Err(FigureAnalysisError::Invalid(format!(
                "unsupported figure type labels schema: {}",
                file.schema_version
            )));
        }
        let default_type = VisualAnalysisType::from_wire(&file.default_type).ok_or_else(|| {
            FigureAnalysisError::Invalid(format!("unknown default_type: {}", file.default_type))
        })?;
        let mut entries = Vec::new();
        for entry in file.types {
            let visual = VisualAnalysisType::from_wire(&entry.wire)
                .or_else(|| VisualAnalysisType::from_wire(&entry.id))
                .ok_or_else(|| {
                    FigureAnalysisError::Invalid(format!(
                        "unknown figure type label: {} / {}",
                        entry.id, entry.wire
                    ))
                })?;
            let keywords = entry
                .caption_keywords
                .into_iter()
                .map(|k| k.trim().to_ascii_lowercase())
                .filter(|k| !k.is_empty())
                .collect::<Vec<_>>();
            entries.push((visual, entry.weight, keywords));
        }
        if entries.is_empty() {
            return Err(FigureAnalysisError::Invalid(
                "figure type labels must declare at least one type".into(),
            ));
        }
        Ok(Self {
            schema_version: file.schema_version,
            default_type,
            entries,
        })
    }

    /// Heuristic Stage-1 classify from caption + citing / section prose.
    pub fn classify_from_text(
        &self,
        artifact_id: &str,
        text: &str,
        context_sha256: &str,
        image_sha256: &str,
    ) -> Result<FigureTypeClassification, FigureAnalysisError> {
        let hay = text.to_ascii_lowercase();
        let mut best_type = self.default_type.clone();
        let mut best_score = 0u32;
        let mut matched = Vec::new();

        for (visual, weight, keywords) in &self.entries {
            let mut hits = Vec::new();
            let mut score = 0u32;
            for kw in keywords {
                if hay.contains(kw.as_str()) {
                    score = score.saturating_add(*weight);
                    hits.push(kw.clone());
                }
            }
            if score > best_score {
                best_score = score;
                best_type = visual.clone();
                matched = hits;
            }
        }

        let confidence = if best_score == 0 {
            0.25
        } else {
            // Soft saturate: 1 hit@weight3 ≈ 0.55, multi-hits approach 0.95.
            let raw = 0.35 + (best_score as f32) * 0.08;
            raw.min(0.95)
        };

        let panels = FigurePanelMarkerTable::bundled()
            .unwrap_or_else(|_| FigurePanelMarkerTable::disabled())
            .detect_panels(text);
        let (subpanel_count, panel_labels) = if panels.is_empty() {
            (1, Vec::new())
        } else {
            (panels.len() as u32, panels)
        };

        let classification = FigureTypeClassification {
            artifact_id: artifact_id.to_string(),
            visual_type: best_type,
            confidence,
            method: FigureClassifyMethod::Heuristic,
            matched_keywords: matched,
            subpanel_count,
            panel_labels,
            labels_schema_version: self.schema_version.clone(),
            context_sha256: context_sha256.to_string(),
            image_sha256: image_sha256.to_string(),
            authoritative: false,
            import_eligible: false,
        };
        classification.validate()?;
        Ok(classification)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FigurePanelMarkersFile {
    schema_version: String,
    caption_first: bool,
    max_panels: u32,
    detectors: Vec<FigurePanelDetectorFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct FigurePanelDetectorFile {
    id: String,
    enabled: bool,
}

/// DATA-driven caption panel markers (ADR-060 / M392).
#[derive(Debug, Clone)]
pub struct FigurePanelMarkerTable {
    pub schema_version: String,
    caption_first: bool,
    max_panels: usize,
    letter_range_parens: bool,
    sequential_letter_parens: bool,
    figure_letter_range: bool,
}

impl FigurePanelMarkerTable {
    pub fn bundled() -> Result<Self, FigureAnalysisError> {
        Self::from_yaml(BUNDLED_FIGURE_PANEL_MARKERS)
    }

    pub fn disabled() -> Self {
        Self {
            schema_version: "figure-panel-markers.v1".into(),
            caption_first: true,
            max_panels: 12,
            letter_range_parens: false,
            sequential_letter_parens: false,
            figure_letter_range: false,
        }
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, FigureAnalysisError> {
        let file: FigurePanelMarkersFile = serde_yaml::from_str(yaml)
            .map_err(|e| FigureAnalysisError::Invalid(format!("figure_panel_markers.yaml: {e}")))?;
        if file.schema_version != "figure-panel-markers.v1" {
            return Err(FigureAnalysisError::Invalid(format!(
                "unsupported figure panel markers schema: {}",
                file.schema_version
            )));
        }
        if file.max_panels == 0 {
            return Err(FigureAnalysisError::Invalid(
                "max_panels must be >= 1".into(),
            ));
        }
        let mut table = Self {
            schema_version: file.schema_version,
            caption_first: file.caption_first,
            max_panels: file.max_panels as usize,
            letter_range_parens: false,
            sequential_letter_parens: false,
            figure_letter_range: false,
        };
        for detector in file.detectors {
            match detector.id.as_str() {
                "letter_range_parens" => table.letter_range_parens = detector.enabled,
                "sequential_letter_parens" => table.sequential_letter_parens = detector.enabled,
                "figure_letter_range" => table.figure_letter_range = detector.enabled,
                other => {
                    return Err(FigureAnalysisError::Invalid(format!(
                        "unknown panel detector: {other}"
                    )));
                }
            }
        }
        Ok(table)
    }

    /// Detect ordered unique panel letters from caption-first text.
    pub fn detect_panels(&self, text: &str) -> Vec<String> {
        let hay = caption_head(text, self.caption_first).to_ascii_lowercase();
        let mut labels = Vec::new();
        if self.letter_range_parens
            && let Some(found) = detect_letter_range_parens(&hay)
        {
            extend_unique(&mut labels, found);
        }
        if self.figure_letter_range
            && let Some(found) = detect_figure_letter_range(&hay)
        {
            extend_unique(&mut labels, found);
        }
        if self.sequential_letter_parens {
            extend_unique(&mut labels, detect_sequential_letter_parens(&hay));
        }
        if labels.len() > self.max_panels {
            labels.truncate(self.max_panels);
        }
        if labels.len() < 2 {
            return Vec::new();
        }
        labels
    }
}

fn caption_head(text: &str, caption_first: bool) -> &str {
    if !caption_first {
        return text;
    }
    text.split_once('.').map(|(head, _)| head).unwrap_or(text)
}

fn extend_unique(dst: &mut Vec<String>, src: Vec<String>) {
    for label in src {
        if !dst.iter().any(|existing| existing == &label) {
            dst.push(label);
        }
    }
}

fn letter_range(start: u8, end: u8) -> Option<Vec<String>> {
    if !start.is_ascii_lowercase() || !end.is_ascii_lowercase() || end < start {
        return None;
    }
    let count = (end - start) as usize + 1;
    if !(2..=12).contains(&count) {
        return None;
    }
    Some((start..=end).map(|c| (c as char).to_string()).collect())
}

fn detect_letter_range_parens(hay: &str) -> Option<Vec<String>> {
    let bytes = hay.as_bytes();
    let mut i = 0;
    while i + 4 < bytes.len() {
        if bytes[i] == b'('
            && bytes[i + 1].is_ascii_lowercase()
            && (bytes[i + 2] == b'-' || bytes[i + 2] == 0xe2)
        {
            // ASCII hyphen or start of en-dash UTF-8 (e2 80 93).
            let (dash_len, end_idx) = if bytes[i + 2] == b'-' {
                (1usize, i + 3)
            } else if i + 5 < bytes.len() && bytes[i + 3] == 0x80 && bytes[i + 4] == 0x93 {
                (3usize, i + 5)
            } else {
                i += 1;
                continue;
            };
            if end_idx < bytes.len()
                && bytes[end_idx].is_ascii_lowercase()
                && end_idx + 1 < bytes.len()
                && bytes[end_idx + 1] == b')'
            {
                let _ = dash_len;
                return letter_range(bytes[i + 1], bytes[end_idx]);
            }
        }
        i += 1;
    }
    None
}

fn detect_figure_letter_range(hay: &str) -> Option<Vec<String>> {
    let lowered = hay;
    let pos = lowered.find("figure").or_else(|| lowered.find("fig."))?;
    let tail = &lowered[pos..];
    let bytes = tail.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i].is_ascii_lowercase()
            && (bytes[i + 1] == b'-' || bytes[i + 1] == 0xe2)
            && i > 0
            && bytes[i - 1].is_ascii_digit()
        {
            let end_idx = if bytes[i + 1] == b'-' {
                i + 2
            } else if i + 4 < bytes.len() && bytes[i + 2] == 0x80 && bytes[i + 3] == 0x93 {
                i + 4
            } else {
                i += 1;
                continue;
            };
            if end_idx < bytes.len() && bytes[end_idx].is_ascii_lowercase() {
                return letter_range(bytes[i], bytes[end_idx]);
            }
        }
        i += 1;
    }
    None
}

fn detect_sequential_letter_parens(hay: &str) -> Vec<String> {
    let bytes = hay.as_bytes();
    let mut labels = Vec::new();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'(' && bytes[i + 1].is_ascii_lowercase() && bytes[i + 2] == b')' {
            let label = (bytes[i + 1] as char).to_string();
            if !labels.iter().any(|existing| existing == &label) {
                labels.push(label);
            }
            i += 3;
            continue;
        }
        i += 1;
    }
    labels
}

/// Selected Stage-2 output schema for a visual type (ADR-060).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureSchemaSelection {
    pub schema_id: String,
    pub description: String,
    pub visual_type: VisualAnalysisType,
    pub required_top_level: Vec<String>,
    pub json_schema: serde_json::Value,
    pub registry_schema_version: String,
}

impl FigureSchemaSelection {
    pub fn validate(&self) -> Result<(), FigureAnalysisError> {
        if self.schema_id.trim().is_empty() {
            return Err(FigureAnalysisError::Missing("schema_id"));
        }
        if self.registry_schema_version.trim().is_empty() {
            return Err(FigureAnalysisError::Missing("registry_schema_version"));
        }
        if !self.json_schema.is_object() {
            return Err(FigureAnalysisError::Invalid(
                "json_schema must be an object".into(),
            ));
        }
        if self.required_top_level.is_empty() {
            return Err(FigureAnalysisError::Invalid(
                "required_top_level must be non-empty".into(),
            ));
        }
        Ok(())
    }

    pub fn schema_sha256_hex(&self) -> Result<String, FigureAnalysisError> {
        let bytes = serde_json::to_vec(&self.json_schema)
            .map_err(|e| FigureAnalysisError::Invalid(format!("schema serialize: {e}")))?;
        use sha2::{Digest, Sha256};
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FigureSchemasFile {
    schema_version: String,
    default_schema_id: String,
    schemas: Vec<FigureSchemaEntryFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct FigureSchemaEntryFile {
    schema_id: String,
    description: String,
    applies_to: Vec<String>,
    required_top_level: Vec<String>,
    json_schema: serde_json::Value,
}

/// DATA-driven registry of per-type Stage-2 figure schemas.
#[derive(Debug, Clone)]
pub struct FigureSchemaRegistry {
    pub schema_version: String,
    default_schema_id: String,
    /// schema_id -> entry
    by_id: std::collections::BTreeMap<String, FigureSchemaEntryFile>,
    /// visual wire type -> schema_id
    type_index: std::collections::BTreeMap<String, String>,
}

impl FigureSchemaRegistry {
    pub fn bundled() -> Result<Self, FigureAnalysisError> {
        Self::from_yaml(BUNDLED_FIGURE_SCHEMAS)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, FigureAnalysisError> {
        let file: FigureSchemasFile = serde_yaml::from_str(yaml)
            .map_err(|e| FigureAnalysisError::Invalid(format!("figure_schemas.yaml: {e}")))?;
        if file.schema_version != "figure-schemas.v1" {
            return Err(FigureAnalysisError::Invalid(format!(
                "unsupported figure schemas version: {}",
                file.schema_version
            )));
        }
        if file.schemas.is_empty() {
            return Err(FigureAnalysisError::Invalid(
                "figure_schemas must declare at least one schema".into(),
            ));
        }
        let mut by_id = std::collections::BTreeMap::new();
        let mut type_index = std::collections::BTreeMap::new();
        for entry in file.schemas {
            if entry.schema_id.trim().is_empty() {
                return Err(FigureAnalysisError::Invalid(
                    "schema_id must be non-empty".into(),
                ));
            }
            if !entry.json_schema.is_object() {
                return Err(FigureAnalysisError::Invalid(format!(
                    "schema {} json_schema must be object",
                    entry.schema_id
                )));
            }
            if entry.required_top_level.is_empty() {
                return Err(FigureAnalysisError::Invalid(format!(
                    "schema {} required_top_level empty",
                    entry.schema_id
                )));
            }
            for wire in &entry.applies_to {
                let key = wire.trim().to_ascii_lowercase();
                if VisualAnalysisType::from_wire(&key).is_none() {
                    return Err(FigureAnalysisError::Invalid(format!(
                        "schema {} applies_to unknown type {wire}",
                        entry.schema_id
                    )));
                }
                if let Some(prev) = type_index.insert(key, entry.schema_id.clone()) {
                    return Err(FigureAnalysisError::Invalid(format!(
                        "visual type {wire} mapped twice ({prev} and {})",
                        entry.schema_id
                    )));
                }
            }
            by_id.insert(entry.schema_id.clone(), entry);
        }
        if !by_id.contains_key(&file.default_schema_id) {
            return Err(FigureAnalysisError::Invalid(format!(
                "default_schema_id {} missing",
                file.default_schema_id
            )));
        }
        Ok(Self {
            schema_version: file.schema_version,
            default_schema_id: file.default_schema_id,
            by_id,
            type_index,
        })
    }

    pub fn select_for(
        &self,
        visual_type: VisualAnalysisType,
    ) -> Result<FigureSchemaSelection, FigureAnalysisError> {
        let wire = visual_type.as_wire();
        let schema_id = self
            .type_index
            .get(wire)
            .cloned()
            .unwrap_or_else(|| self.default_schema_id.clone());
        let entry = self.by_id.get(&schema_id).ok_or_else(|| {
            FigureAnalysisError::Invalid(format!("schema_id {schema_id} missing"))
        })?;
        let selection = FigureSchemaSelection {
            schema_id: entry.schema_id.clone(),
            description: entry.description.clone(),
            visual_type,
            required_top_level: entry.required_top_level.clone(),
            json_schema: entry.json_schema.clone(),
            registry_schema_version: self.schema_version.clone(),
        };
        selection.validate()?;
        Ok(selection)
    }

    pub fn schema_ids(&self) -> Vec<String> {
        self.by_id.keys().cloned().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureAnalysisComponent {
    pub name: String,
    pub role: String,
    pub visual_evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureAnalysisRelationship {
    pub source_component: String,
    pub relation: String,
    pub target_component: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureAnalysisCitation {
    pub section_id: String,
    pub path_id: String,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureAnalysisProvenance {
    pub image_sha256: String,
    pub context_sha256: String,
    pub route_id: String,
    pub protocol: String,
    pub requested_model: String,
    pub resolved_model: String,
    /// Registry-bound prompt identity (S10). Empty for legacy readers that
    /// predate the prompt registry binding.
    #[serde(default)]
    pub prompt_id: String,
    /// Registry-bound prompt version (S10). Empty for legacy readers.
    #[serde(default)]
    pub prompt_version: String,
    pub prompt_sha256: String,
    pub output_schema_sha256: String,
    /// DATA Stage-2 schema_id selected for this candidate (ADR-060 / M389).
    pub schema_id: String,
    /// Stage-1 visual type wire string used for schema routing (may differ from draft).
    pub stage1_visual_type: String,
    pub prompt_task_sha256: String,
    pub route_policy_sha256: String,
    pub model_binding_sha256: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub latency_ms: u64,
}

/// Candidate-only typed Stage-2 payload (ADR-060 / M390–M391).
/// Chart, table, and photo drafts do not invent diagram components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FigureTypedPayload {
    Chart(FigureChartPayload),
    Table(FigureTablePayload),
    Photo(FigurePhotoPayload),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureChartAxis {
    pub label: String,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureChartDataPoint {
    pub label: String,
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureChartPayload {
    pub chart_type: String,
    pub x_axis: FigureChartAxis,
    pub y_axis: FigureChartAxis,
    #[serde(default)]
    pub key_data_points: Vec<FigureChartDataPoint>,
    #[serde(default)]
    pub trend: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureTablePayload {
    pub headers: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureScaleBar {
    pub present: bool,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigurePhotoPayload {
    pub visible_objects: Vec<String>,
    #[serde(default)]
    pub scale_bar: Option<FigureScaleBar>,
}

impl FigureTypedPayload {
    pub fn validate(&self) -> Result<(), FigureAnalysisError> {
        match self {
            Self::Chart(chart) => {
                if chart.chart_type.trim().is_empty() {
                    return Err(FigureAnalysisError::Missing("chart_type"));
                }
                if chart.x_axis.label.trim().is_empty() {
                    return Err(FigureAnalysisError::Missing("x_axis.label"));
                }
                if chart.y_axis.label.trim().is_empty() {
                    return Err(FigureAnalysisError::Missing("y_axis.label"));
                }
                if chart
                    .key_data_points
                    .iter()
                    .any(|p| p.label.trim().is_empty())
                {
                    return Err(FigureAnalysisError::Invalid(
                        "key_data_points require a non-empty label".into(),
                    ));
                }
            }
            Self::Table(table) => {
                if table.headers.is_empty() || table.headers.iter().any(|h| h.trim().is_empty()) {
                    return Err(FigureAnalysisError::Invalid(
                        "table headers must be non-empty".into(),
                    ));
                }
            }
            Self::Photo(photo) => {
                if photo.visible_objects.is_empty()
                    || photo.visible_objects.iter().any(|o| o.trim().is_empty())
                {
                    return Err(FigureAnalysisError::Invalid(
                        "photo visible_objects must be non-empty".into(),
                    ));
                }
                if let Some(bar) = &photo.scale_bar
                    && bar.present
                {
                    let empty = bar.value.as_deref().map(str::trim).unwrap_or("").is_empty();
                    if empty {
                        return Err(FigureAnalysisError::Invalid(
                            "scale_bar.value required when present=true".into(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FigureQuotePolicy {
    /// Guideline cap the model is nudged toward. Exceeding it is legal and
    /// measured, never a rejection by itself (ADR-062 dynamic window).
    pub soft_target_chars: u32,
    /// Hard schema ceiling. JSON-schema maxLength equals this value;
    /// beyond it validation stays terminal (fail-closed).
    pub absolute_max_chars: u32,
}

impl FigureQuotePolicy {
    /// 1 <= soft <= absolute; absolute bounded so candidates stay storable.
    pub fn validate(&self) -> Result<(), String> {
        if self.soft_target_chars == 0 {
            return Err("figure quote soft target must be positive".into());
        }
        if self.soft_target_chars > self.absolute_max_chars {
            return Err(format!(
                "figure quote soft target {} exceeds absolute max {}",
                self.soft_target_chars, self.absolute_max_chars
            ));
        }
        if self.absolute_max_chars > 8_000 {
            return Err(format!(
                "figure quote absolute max {} above storable bound 8000",
                self.absolute_max_chars
            ));
        }
        Ok(())
    }

    pub fn fingerprint(&self) -> String {
        format!(
            "quote_policy=soft={},abs={}",
            self.soft_target_chars, self.absolute_max_chars
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureAnalysis {
    pub candidate_id: String,
    pub artifact_id: String,
    pub visual_type: VisualAnalysisType,
    pub description: String,
    pub components: Vec<FigureAnalysisComponent>,
    pub relationships: Vec<FigureAnalysisRelationship>,
    pub findings: Vec<String>,
    pub citations: Vec<FigureAnalysisCitation>,
    pub confidence: f32,
    pub provenance: FigureAnalysisProvenance,
    /// Optional typed Stage-2 payload. When present, diagram components may be empty.
    #[serde(default)]
    pub typed_payload: Option<FigureTypedPayload>,
    pub authoritative: bool,
    pub import_eligible: bool,
}

impl FigureAnalysis {
    pub fn validate(&self) -> Result<(), FigureAnalysisError> {
        for (field, value) in [
            ("candidate_id", self.candidate_id.as_str()),
            ("artifact_id", self.artifact_id.as_str()),
            ("description", self.description.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(FigureAnalysisError::Missing(field));
            }
        }
        if let Some(payload) = &self.typed_payload {
            payload.validate()?;
        }
        if self.components.is_empty() && self.typed_payload.is_none() {
            return Err(FigureAnalysisError::Missing("components"));
        }
        if self.components.iter().any(|component| {
            component.name.trim().is_empty()
                || component.role.trim().is_empty()
                || component.visual_evidence.trim().is_empty()
        }) {
            return Err(FigureAnalysisError::Invalid(
                "components require name, role and visual_evidence".into(),
            ));
        }
        let component_names: HashSet<_> = self
            .components
            .iter()
            .map(|component| component.name.as_str())
            .collect();
        if component_names.len() != self.components.len() {
            return Err(FigureAnalysisError::Invalid(
                "component names must be unique".into(),
            ));
        }
        if self.relationships.iter().any(|relationship| {
            relationship.relation.trim().is_empty()
                || !component_names.contains(relationship.source_component.as_str())
                || !component_names.contains(relationship.target_component.as_str())
        }) {
            return Err(FigureAnalysisError::Invalid(
                "relationship endpoints must reference declared components".into(),
            ));
        }
        if self
            .findings
            .iter()
            .any(|finding| finding.trim().is_empty())
        {
            return Err(FigureAnalysisError::Invalid(
                "findings cannot contain empty values".into(),
            ));
        }
        if self.citations.is_empty()
            || self.citations.iter().any(|citation| {
                citation.section_id.trim().is_empty()
                    || citation.path_id.trim().is_empty()
                    || citation.quote.trim().is_empty()
            })
        {
            return Err(FigureAnalysisError::Invalid(
                "at least one complete context citation is required".into(),
            ));
        }
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(FigureAnalysisError::Invalid(
                "confidence must be finite and between 0 and 1".into(),
            ));
        }
        for (field, value) in [
            ("schema_id", self.provenance.schema_id.as_str()),
            (
                "stage1_visual_type",
                self.provenance.stage1_visual_type.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return Err(FigureAnalysisError::Missing(field));
            }
        }
        if VisualAnalysisType::from_wire(&self.provenance.stage1_visual_type).is_none() {
            return Err(FigureAnalysisError::Invalid(format!(
                "unknown stage1_visual_type: {}",
                self.provenance.stage1_visual_type
            )));
        }
        for (field, value) in [
            ("image_sha256", &self.provenance.image_sha256),
            ("context_sha256", &self.provenance.context_sha256),
            ("prompt_sha256", &self.provenance.prompt_sha256),
            (
                "output_schema_sha256",
                &self.provenance.output_schema_sha256,
            ),
            ("prompt_task_sha256", &self.provenance.prompt_task_sha256),
            ("route_policy_sha256", &self.provenance.route_policy_sha256),
            (
                "model_binding_sha256",
                &self.provenance.model_binding_sha256,
            ),
            ("input_sha256", &self.provenance.input_sha256),
            ("output_sha256", &self.provenance.output_sha256),
        ] {
            if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(FigureAnalysisError::Invalid(format!(
                    "{field} must be a SHA-256 digest"
                )));
            }
        }
        if self.authoritative || self.import_eligible {
            return Err(FigureAnalysisError::AuthorityViolation);
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FigureAnalysisError {
    #[error("missing figure analysis field: {0}")]
    Missing(&'static str),
    #[error("invalid figure analysis: {0}")]
    Invalid(String),
    #[error("figure analysis candidate cannot be authoritative or import eligible")]
    AuthorityViolation,
    #[error("ungrounded context citation: {0}")]
    UngroundedCitation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figure_quote_policy_accepts_valid_window() {
        let policy = FigureQuotePolicy {
            soft_target_chars: 1500,
            absolute_max_chars: 4000,
        };
        assert_eq!(policy.validate(), Ok(()));
    }

    #[test]
    fn figure_quote_policy_rejects_inverted_or_absurd_windows() {
        let inverted = FigureQuotePolicy {
            soft_target_chars: 4000,
            absolute_max_chars: 1500,
        };
        assert!(inverted.validate().is_err());
        let zero_soft = FigureQuotePolicy {
            soft_target_chars: 0,
            absolute_max_chars: 4000,
        };
        assert!(zero_soft.validate().is_err());
        let absurd_ceiling = FigureQuotePolicy {
            soft_target_chars: 1500,
            absolute_max_chars: 9000,
        };
        assert!(absurd_ceiling.validate().is_err());
    }

    #[test]
    fn figure_quote_policy_denies_unknown_fields() {
        let text = serde_yaml::to_string(&FigureQuotePolicy {
            soft_target_chars: 1500,
            absolute_max_chars: 4000,
        })
        .unwrap();
        let patched = format!("{text}extra_field: 1\n");
        assert!(serde_yaml::from_str::<FigureQuotePolicy>(&patched).is_err());
    }

    fn candidate() -> FigureAnalysis {
        FigureAnalysis {
            candidate_id: "figure-analysis:2507.19457:Figure3:v1".into(),
            artifact_id: "Figure3".into(),
            visual_type: VisualAnalysisType::Flowchart,
            description: "GEPA optimization flowchart".into(),
            components: vec![FigureAnalysisComponent {
                name: "Reflective mutation".into(),
                role: "Proposes prompt variants".into(),
                visual_evidence: "mutation arrow".into(),
            }],
            relationships: vec![],
            findings: vec!["Candidate evaluation feeds Pareto selection".into()],
            citations: vec![FigureAnalysisCitation {
                section_id: "sec-3".into(),
                path_id: "3".into(),
                quote: "GEPA mutates prompts".into(),
            }],
            confidence: 0.9,
            provenance: FigureAnalysisProvenance {
                image_sha256: "a".repeat(64),
                context_sha256: "b".repeat(64),
                route_id: "minimax-direct".into(),
                protocol: "anthropic_messages".into(),
                requested_model: "MiniMax-M3-512k".into(),
                resolved_model: "MiniMax-M3".into(),
                prompt_id: "figure_analysis".into(),
                prompt_version: "figure-analysis.v3".into(),
                prompt_sha256: "c".repeat(64),
                output_schema_sha256: "d".repeat(64),
                schema_id: "diagram_analysis".into(),
                stage1_visual_type: "flowchart".into(),
                prompt_task_sha256: "e".repeat(64),
                route_policy_sha256: "f".repeat(64),
                model_binding_sha256: "1".repeat(64),
                input_sha256: "2".repeat(64),
                output_sha256: "3".repeat(64),
                input_tokens: Some(100),
                output_tokens: Some(30),
                latency_ms: 500,
            },
            typed_payload: None,
            authoritative: false,
            import_eligible: false,
        }
    }

    #[test]
    fn chart_payload_allows_empty_components() {
        let mut value = candidate();
        value.visual_type = VisualAnalysisType::Plot;
        value.components.clear();
        value.relationships.clear();
        value.provenance.schema_id = "chart_extraction".into();
        value.provenance.stage1_visual_type = "plot".into();
        value.typed_payload = Some(FigureTypedPayload::Chart(FigureChartPayload {
            chart_type: "scatter".into(),
            x_axis: FigureChartAxis {
                label: "rollouts".into(),
                unit: None,
            },
            y_axis: FigureChartAxis {
                label: "accuracy".into(),
                unit: Some("%".into()),
            },
            key_data_points: vec![FigureChartDataPoint {
                label: "GEPA".into(),
                x: Some("100".into()),
                y: Some("0.9".into()),
            }],
            trend: None,
        }));
        value.validate().unwrap();
        let decoded: FigureAnalysis =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn table_payload_allows_empty_components() {
        let mut value = candidate();
        value.visual_type = VisualAnalysisType::Table;
        value.components.clear();
        value.relationships.clear();
        value.provenance.schema_id = "table_structure".into();
        value.provenance.stage1_visual_type = "table".into();
        value.typed_payload = Some(FigureTypedPayload::Table(FigureTablePayload {
            headers: vec!["method".into(), "f1".into()],
            rows: vec![vec!["GEPA".into(), "0.87".into()]],
        }));
        value.validate().unwrap();
    }

    #[test]
    fn photo_payload_allows_empty_components() {
        let mut value = candidate();
        value.visual_type = VisualAnalysisType::Photograph;
        value.components.clear();
        value.relationships.clear();
        value.provenance.schema_id = "photo_caption".into();
        value.provenance.stage1_visual_type = "photograph".into();
        value.typed_payload = Some(FigureTypedPayload::Photo(FigurePhotoPayload {
            visible_objects: vec!["scale bar".into(), "cell monolayer".into()],
            scale_bar: Some(FigureScaleBar {
                present: true,
                value: Some("10 um".into()),
            }),
        }));
        value.validate().unwrap();
    }

    #[test]
    fn photo_payload_rejects_empty_visible_objects() {
        let mut value = candidate();
        value.components.clear();
        value.typed_payload = Some(FigureTypedPayload::Photo(FigurePhotoPayload {
            visible_objects: vec![],
            scale_bar: None,
        }));
        assert_eq!(
            value.validate(),
            Err(FigureAnalysisError::Invalid(
                "photo visible_objects must be non-empty".into(),
            ))
        );
    }

    #[test]
    fn photo_payload_requires_scale_bar_value_when_present() {
        let mut value = candidate();
        value.components.clear();
        value.typed_payload = Some(FigureTypedPayload::Photo(FigurePhotoPayload {
            visible_objects: vec!["colony".into()],
            scale_bar: Some(FigureScaleBar {
                present: true,
                value: None,
            }),
        }));
        assert!(value.validate().is_err());
    }

    #[test]
    fn diagram_without_components_or_payload_still_fails() {
        let mut value = candidate();
        value.components.clear();
        assert_eq!(
            value.validate(),
            Err(FigureAnalysisError::Missing("components"))
        );
    }

    #[test]
    fn chart_payload_requires_axis_labels() {
        let mut value = candidate();
        value.components.clear();
        value.typed_payload = Some(FigureTypedPayload::Chart(FigureChartPayload {
            chart_type: "bar".into(),
            x_axis: FigureChartAxis {
                label: "".into(),
                unit: None,
            },
            y_axis: FigureChartAxis {
                label: "score".into(),
                unit: None,
            },
            key_data_points: vec![],
            trend: None,
        }));
        assert_eq!(
            value.validate(),
            Err(FigureAnalysisError::Missing("x_axis.label"))
        );
    }

    #[test]
    fn candidate_roundtrips_and_stays_non_authoritative() {
        let value = candidate();
        value.validate().unwrap();
        let decoded: FigureAnalysis =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn rejects_unknown_relationship_endpoints() {
        let mut value = candidate();
        value.relationships.push(FigureAnalysisRelationship {
            source_component: "Reflective mutation".into(),
            relation: "feeds".into(),
            target_component: "Invented component".into(),
        });
        assert!(matches!(
            value.validate(),
            Err(FigureAnalysisError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_authority_promotion_and_invalid_confidence() {
        let mut value = candidate();
        value.authoritative = true;
        assert_eq!(
            value.validate(),
            Err(FigureAnalysisError::AuthorityViolation)
        );
        value.authoritative = false;
        value.confidence = f32::NAN;
        assert!(matches!(
            value.validate(),
            Err(FigureAnalysisError::Invalid(_))
        ));
    }

    fn sha64(seed: u8) -> String {
        format!("{:064x}", seed as u128)
    }

    #[test]
    fn bundled_labels_load_and_classify_flowchart_caption() {
        let table = FigureTypeLabelTable::bundled().expect("bundled labels");
        let c = table
            .classify_from_text(
                "Figure3",
                "Figure 3: Overview of the GEPA algorithm candidate proposal loop with reflective prompt mutation and system aware merge",
                &sha64(1),
                &sha64(2),
            )
            .expect("classify");
        assert_eq!(c.visual_type, VisualAnalysisType::Flowchart);
        assert!(c.confidence >= 0.5);
        assert_eq!(c.method, FigureClassifyMethod::Heuristic);
        assert!(!c.matched_keywords.is_empty());
        assert!(!c.authoritative && !c.import_eligible);
        c.validate().unwrap();
    }

    #[test]
    fn bundled_labels_classify_plot_caption() {
        let table = FigureTypeLabelTable::bundled().expect("bundled labels");
        let c = table
            .classify_from_text(
                "Figure2",
                "Figure 2: Performance comparison scatter plot of accuracy vs rollouts",
                &sha64(3),
                &sha64(4),
            )
            .expect("classify");
        assert_eq!(c.visual_type, VisualAnalysisType::Plot);
        assert_eq!(c.method, FigureClassifyMethod::Heuristic);
        assert_eq!(c.subpanel_count, 1);
        assert!(c.panel_labels.is_empty());
    }

    #[test]
    fn compound_caption_letter_range_sets_panel_count() {
        let table = FigureTypeLabelTable::bundled().expect("bundled labels");
        let c = table
            .classify_from_text(
                "Figure3",
                "Figure 3(a-c): Overview of the algorithm candidate proposal loop.",
                &sha64(7),
                &sha64(8),
            )
            .expect("classify");
        assert_eq!(c.subpanel_count, 3);
        assert_eq!(c.panel_labels, vec!["a", "b", "c"]);
        c.validate().unwrap();
    }

    #[test]
    fn compound_caption_sequential_letters_sets_panel_count() {
        let table = FigureTypeLabelTable::bundled().expect("bundled labels");
        let c = table
            .classify_from_text(
                "Figure4",
                "Figure 4: (a) train (b) validate (c) test accuracy vs rollouts plot.",
                &sha64(9),
                &sha64(10),
            )
            .expect("classify");
        assert_eq!(c.visual_type, VisualAnalysisType::Plot);
        assert_eq!(c.subpanel_count, 3);
        assert_eq!(c.panel_labels, vec!["a", "b", "c"]);
    }

    #[test]
    fn figure_letter_range_after_number_sets_panel_count() {
        let markers = FigurePanelMarkerTable::bundled().expect("markers");
        let labels = markers.detect_panels("Figure 3a-c shows the pipeline overview.");
        assert_eq!(labels, vec!["a", "b", "c"]);
    }

    #[test]
    fn single_panel_caption_does_not_invent_panels() {
        let markers = FigurePanelMarkerTable::bundled().expect("markers");
        assert!(
            markers
                .detect_panels("Figure 3: Overview of the GEPA algorithm.")
                .is_empty()
        );
        assert!(
            markers
                .detect_panels("Results in section 3a are discussed.")
                .is_empty()
        );
    }

    #[test]
    fn stage1_classification_rejects_authoritative_flag() {
        let mut c = FigureTypeClassification {
            artifact_id: "Figure1".into(),
            visual_type: VisualAnalysisType::Other,
            confidence: 0.25,
            method: FigureClassifyMethod::Heuristic,
            matched_keywords: vec![],
            subpanel_count: 1,
            panel_labels: vec![],
            labels_schema_version: "figure-type-labels.v1".into(),
            context_sha256: sha64(5),
            image_sha256: sha64(6),
            authoritative: false,
            import_eligible: false,
        };
        c.validate().unwrap();
        c.authoritative = true;
        assert_eq!(c.validate(), Err(FigureAnalysisError::AuthorityViolation));
    }

    #[test]
    fn visual_type_wire_roundtrip() {
        for t in [
            VisualAnalysisType::Flowchart,
            VisualAnalysisType::Plot,
            VisualAnalysisType::Diagram,
            VisualAnalysisType::Table,
            VisualAnalysisType::Screenshot,
            VisualAnalysisType::Photograph,
            VisualAnalysisType::Other,
        ] {
            assert_eq!(VisualAnalysisType::from_wire(t.as_wire()), Some(t));
        }
    }

    #[test]
    fn bundled_figure_schemas_select_by_type() {
        let reg = FigureSchemaRegistry::bundled().expect("bundled schemas");
        assert!(reg.schema_ids().contains(&"diagram_analysis".into()));
        assert!(reg.schema_ids().contains(&"chart_extraction".into()));
        assert!(reg.schema_ids().contains(&"table_structure".into()));

        let flowchart = reg
            .select_for(VisualAnalysisType::Flowchart)
            .expect("flowchart schema");
        assert_eq!(flowchart.schema_id, "diagram_analysis");
        assert!(
            flowchart
                .required_top_level
                .iter()
                .any(|f| f == "components")
        );
        flowchart.validate().unwrap();
        assert_eq!(flowchart.schema_sha256_hex().unwrap().len(), 64);

        let plot = reg.select_for(VisualAnalysisType::Plot).expect("plot");
        assert_eq!(plot.schema_id, "chart_extraction");
        assert!(
            plot.required_top_level
                .iter()
                .any(|f| f == "key_data_points")
        );

        let table = reg.select_for(VisualAnalysisType::Table).expect("table");
        assert_eq!(table.schema_id, "table_structure");

        let photo = reg
            .select_for(VisualAnalysisType::Photograph)
            .expect("photo");
        assert_eq!(photo.schema_id, "photo_caption");

        let other = reg.select_for(VisualAnalysisType::Other).expect("other");
        assert_eq!(other.schema_id, "other_description");
    }

    #[test]
    fn figure_schemas_reject_bad_version() {
        let yaml = r#"
schema_version: figure-schemas.v999
import_eligible: false
default_schema_id: x
schemas: []
"#;
        assert!(FigureSchemaRegistry::from_yaml(yaml).is_err());
    }
}
