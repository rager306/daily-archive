//! FSM policy types (ADR-074 Layer 1 + Layer 2 contract).
//!
//! Pure domain: article features in, named decision out. No WASM, no IO.
//! Native threshold rules live here as data-driven functions; the YAML
//! registry is parsed by [`PolicyRegistry::from_yaml_str`]. WASM hosts
//! live in da-adapters and must produce the same [`PolicyDecision`] shape.

use serde::{Deserialize, Serialize};

/// Closed set of FSM decision points that may consult a policy module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionPoint {
    GrammarRoute,
    BatchAdmission,
    ConcurrencyTune,
    RetryPolicy,
    SummaryDepth,
    ProfileRoute,
}

impl DecisionPoint {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GrammarRoute => "grammar_route",
            Self::BatchAdmission => "batch_admission",
            Self::ConcurrencyTune => "concurrency_tune",
            Self::RetryPolicy => "retry_policy",
            Self::SummaryDepth => "summary_depth",
            Self::ProfileRoute => "profile_route",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "grammar_route" => Some(Self::GrammarRoute),
            "batch_admission" => Some(Self::BatchAdmission),
            "concurrency_tune" => Some(Self::ConcurrencyTune),
            "retry_policy" => Some(Self::RetryPolicy),
            "summary_depth" => Some(Self::SummaryDepth),
            "profile_route" => Some(Self::ProfileRoute),
            _ => None,
        }
    }
}

/// Numeric article features the policy is allowed to see.
/// No raw text — keeps the WASM sandbox contract small and deterministic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleFeatures {
    pub page_count: u32,
    pub toc_bookmarks: u32,
    /// 0.0..=1.0 text-layer quality (1.0 = selectable text, 0.0 = scan).
    pub text_layer_quality: f32,
    /// Domain id from `data/domains/*/SCHEMA.yaml` (empty = unknown).
    #[serde(default)]
    pub domain: String,
}

impl ArticleFeatures {
    pub fn quality_milli(&self) -> i32 {
        (self.text_layer_quality.clamp(0.0, 1.0) * 1000.0).round() as i32
    }
}

/// Grammar chosen for the parse stage (ADR-074 grammar_route).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrammarRoute {
    PageOutline,
    XyCut,
    GrobidStructural,
}

impl GrammarRoute {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PageOutline => "page_outline",
            Self::XyCut => "xy_cut",
            Self::GrobidStructural => "grobid_structural",
        }
    }

    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::PageOutline),
            1 => Some(Self::XyCut),
            2 => Some(Self::GrobidStructural),
            _ => None,
        }
    }

    pub fn to_code(self) -> i32 {
        match self {
            Self::PageOutline => 0,
            Self::XyCut => 1,
            Self::GrobidStructural => 2,
        }
    }
}

/// Java GROBID REST profile (ADR-076 profile_route). Not FSM grammar_profile.
///
/// Closed set. Corpus default is [`ProfileRoute::Structural`] (D204: no
/// Crossref inside the structural fingerprint). Rich is a named variant,
/// never implied by grammar_route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileRoute {
    Structural,
    Rich,
}

impl ProfileRoute {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::Rich => "rich",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "structural" => Some(Self::Structural),
            "rich" => Some(Self::Rich),
            _ => None,
        }
    }

    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Structural),
            1 => Some(Self::Rich),
            _ => None,
        }
    }

    pub fn to_code(self) -> i32 {
        match self {
            Self::Structural => 0,
            Self::Rich => 1,
        }
    }
}

/// Batch admission (ADR-074 batch_admission).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Admission {
    Accept,
    Defer,
    Skip,
}

impl Admission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Defer => "defer",
            Self::Skip => "skip",
        }
    }

    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Accept),
            1 => Some(Self::Defer),
            2 => Some(Self::Skip),
            _ => None,
        }
    }
}

/// Who produced the decision — native fallback or a versioned WASM artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicySource {
    Native,
    Wasm { sha256: String },
}

impl PolicySource {
    pub fn hash_stamp(&self) -> &str {
        match self {
            Self::Native => "native",
            Self::Wasm { sha256 } => sha256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyId {
    pub decision_point: String,
    pub source: PolicySource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub id: PolicyId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<GrammarRoute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission: Option<Admission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileRoute>,
}

impl PolicyDecision {
    pub fn grammar(source: PolicySource, route: GrammarRoute) -> Self {
        Self {
            id: PolicyId {
                decision_point: DecisionPoint::GrammarRoute.as_str().to_string(),
                source,
            },
            route: Some(route),
            admission: None,
            profile: None,
        }
    }

    pub fn admission(source: PolicySource, admission: Admission) -> Self {
        Self {
            id: PolicyId {
                decision_point: DecisionPoint::BatchAdmission.as_str().to_string(),
                source,
            },
            route: None,
            admission: Some(admission),
            profile: None,
        }
    }

    pub fn profile(source: PolicySource, profile: ProfileRoute) -> Self {
        Self {
            id: PolicyId {
                decision_point: DecisionPoint::ProfileRoute.as_str().to_string(),
                source,
            },
            route: None,
            admission: None,
            profile: Some(profile),
        }
    }
}

/// Native grammar_route thresholds (from DATA registry, not hardcoded in adapters).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrammarRouteNative {
    #[serde(default = "default_page_outline_min_pages")]
    pub page_outline_min_pages: u32,
    #[serde(default = "default_page_outline_min_bookmarks")]
    pub page_outline_min_bookmarks: u32,
    #[serde(default = "default_grobid_max_text_quality")]
    pub grobid_max_text_quality: f32,
}

fn default_page_outline_min_pages() -> u32 {
    50
}
fn default_page_outline_min_bookmarks() -> u32 {
    1
}
fn default_grobid_max_text_quality() -> f32 {
    0.4
}

impl Default for GrammarRouteNative {
    fn default() -> Self {
        Self {
            page_outline_min_pages: default_page_outline_min_pages(),
            page_outline_min_bookmarks: default_page_outline_min_bookmarks(),
            grobid_max_text_quality: default_grobid_max_text_quality(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchAdmissionNative {
    #[serde(default)]
    pub skip_if_pages_eq: u32,
    #[serde(default = "default_defer_max_text_quality")]
    pub defer_max_text_quality: f32,
}

fn default_defer_max_text_quality() -> f32 {
    0.2
}

impl Default for BatchAdmissionNative {
    fn default() -> Self {
        Self {
            skip_if_pages_eq: 0,
            defer_max_text_quality: default_defer_max_text_quality(),
        }
    }
}

/// Native profile_route (DATA; corpus default is structural / D204).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileRouteNative {
    #[serde(default = "default_profile_route")]
    pub default: ProfileRoute,
}

fn default_profile_route() -> ProfileRoute {
    ProfileRoute::Structural
}

impl Default for ProfileRouteNative {
    fn default() -> Self {
        Self {
            default: default_profile_route(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionPointSpec {
    #[serde(default)]
    pub wasm: Option<String>,
    #[serde(default)]
    pub wat: Option<String>,
    #[serde(default)]
    pub native: serde_json::Value,
    /// GrammarRoute.as_str() -> FSM grammar_profile (DATA; parse-pdf consult).
    #[serde(default)]
    pub profile_map: std::collections::BTreeMap<String, String>,
    /// Fail-open FSM grammar_profile when route is missing from the map.
    #[serde(default)]
    pub default_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyRegistry {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub adr_ref: String,
    #[serde(default)]
    pub decision_points: std::collections::BTreeMap<String, DecisionPointSpec>,
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyRegistryError {
    #[error("policy registry: YAML parse failed: {0}")]
    Parse(String),
}

impl PolicyRegistry {
    pub fn from_yaml_str(yaml: &str) -> Result<Self, PolicyRegistryError> {
        serde_yaml::from_str(yaml).map_err(|e| PolicyRegistryError::Parse(e.to_string()))
    }

    pub fn grammar_native(&self) -> GrammarRouteNative {
        self.decision_points
            .get(DecisionPoint::GrammarRoute.as_str())
            .and_then(|s| serde_json::from_value(s.native.clone()).ok())
            .unwrap_or_default()
    }

    pub fn admission_native(&self) -> BatchAdmissionNative {
        self.decision_points
            .get(DecisionPoint::BatchAdmission.as_str())
            .and_then(|s| serde_json::from_value(s.native.clone()).ok())
            .unwrap_or_default()
    }

    pub fn profile_route_native(&self) -> ProfileRouteNative {
        self.decision_points
            .get(DecisionPoint::ProfileRoute.as_str())
            .and_then(|s| serde_json::from_value(s.native.clone()).ok())
            .unwrap_or_default()
    }

    pub fn spec(&self, point: DecisionPoint) -> Option<&DecisionPointSpec> {
        self.decision_points.get(point.as_str())
    }

    /// Map a GrammarRoute to an FSM grammar_profile via DATA profile_map.
    /// Missing keys fall open to default_profile, then empty (caller decides).
    pub fn profile_for(&self, route: GrammarRoute) -> Option<&str> {
        let spec = self.spec(DecisionPoint::GrammarRoute)?;
        spec.profile_map
            .get(route.as_str())
            .map(String::as_str)
            .or(spec.default_profile.as_deref())
    }

    pub fn default_grammar_profile(&self) -> Option<&str> {
        self.spec(DecisionPoint::GrammarRoute)
            .and_then(|s| s.default_profile.as_deref())
    }
}

/// Deterministic native grammar_route (fail-open to XyCut).
pub fn grammar_route_native(features: &ArticleFeatures, cfg: &GrammarRouteNative) -> GrammarRoute {
    if features.page_count >= cfg.page_outline_min_pages
        && features.toc_bookmarks >= cfg.page_outline_min_bookmarks
    {
        return GrammarRoute::PageOutline;
    }
    if features.text_layer_quality < cfg.grobid_max_text_quality {
        return GrammarRoute::GrobidStructural;
    }
    GrammarRoute::XyCut
}

/// Deterministic native profile_route (fail-open to structural / D204).
/// Features are unused this cut: corpus must not infer rich from page count.
pub fn profile_route_native(_features: &ArticleFeatures, cfg: &ProfileRouteNative) -> ProfileRoute {
    cfg.default
}

/// Deterministic native batch_admission (fail-open to Accept).
pub fn batch_admission_native(features: &ArticleFeatures, cfg: &BatchAdmissionNative) -> Admission {
    if features.page_count == cfg.skip_if_pages_eq {
        return Admission::Skip;
    }
    if features.text_layer_quality < cfg.defer_max_text_quality {
        return Admission::Defer;
    }
    Admission::Accept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feat(pages: u32, bookmarks: u32, quality: f32) -> ArticleFeatures {
        ArticleFeatures {
            page_count: pages,
            toc_bookmarks: bookmarks,
            text_layer_quality: quality,
            domain: "computer_science".into(),
        }
    }

    #[test]
    fn monograph_with_toc_routes_page_outline() {
        let r = grammar_route_native(&feat(700, 40, 0.9), &GrammarRouteNative::default());
        assert_eq!(r, GrammarRoute::PageOutline);
    }

    #[test]
    fn low_text_quality_routes_grobid() {
        let r = grammar_route_native(&feat(12, 0, 0.1), &GrammarRouteNative::default());
        assert_eq!(r, GrammarRoute::GrobidStructural);
    }

    #[test]
    fn typical_article_routes_xy_cut() {
        let r = grammar_route_native(&feat(18, 0, 0.85), &GrammarRouteNative::default());
        assert_eq!(r, GrammarRoute::XyCut);
    }

    #[test]
    fn zero_pages_skipped() {
        let a = batch_admission_native(&feat(0, 0, 1.0), &BatchAdmissionNative::default());
        assert_eq!(a, Admission::Skip);
    }

    #[test]
    fn scan_deferred() {
        let a = batch_admission_native(&feat(10, 0, 0.05), &BatchAdmissionNative::default());
        assert_eq!(a, Admission::Defer);
    }

    #[test]
    fn healthy_article_accepted() {
        let a = batch_admission_native(&feat(10, 0, 0.9), &BatchAdmissionNative::default());
        assert_eq!(a, Admission::Accept);
    }

    #[test]
    fn registry_parses_native_thresholds() {
        let yaml = r#"
schema_version: v2
adr_ref: ADR-074
decision_points:
  grammar_route:
    native:
      page_outline_min_pages: 80
      page_outline_min_bookmarks: 2
      grobid_max_text_quality: 0.3
  batch_admission:
    native:
      skip_if_pages_eq: 0
      defer_max_text_quality: 0.15
"#;
        let reg = PolicyRegistry::from_yaml_str(yaml).unwrap();
        let g = reg.grammar_native();
        assert_eq!(g.page_outline_min_pages, 80);
        assert_eq!(g.page_outline_min_bookmarks, 2);
        let r = grammar_route_native(&feat(70, 5, 0.9), &g);
        assert_eq!(r, GrammarRoute::XyCut, "70 pages < 80 threshold");
        let r2 = grammar_route_native(&feat(80, 2, 0.9), &g);
        assert_eq!(r2, GrammarRoute::PageOutline);
    }

    #[test]
    fn profile_map_from_yaml_maps_routes() {
        let yaml = r#"
decision_points:
  grammar_route:
    default_profile: arxiv_pdf_dot
    profile_map:
      page_outline: page_outline
      xy_cut: arxiv_pdf_dot
      grobid_structural: arxiv_pdf_dot
"#;
        let reg = PolicyRegistry::from_yaml_str(yaml).unwrap();
        assert_eq!(
            reg.profile_for(GrammarRoute::PageOutline),
            Some("page_outline")
        );
        assert_eq!(reg.profile_for(GrammarRoute::XyCut), Some("arxiv_pdf_dot"));
        assert_eq!(reg.default_grammar_profile(), Some("arxiv_pdf_dot"));
    }

    #[test]
    fn unknown_decision_point_rejected() {
        assert!(DecisionPoint::parse("invented_route").is_none());
        assert_eq!(
            DecisionPoint::parse("grammar_route"),
            Some(DecisionPoint::GrammarRoute)
        );
        assert_eq!(
            DecisionPoint::parse("profile_route"),
            Some(DecisionPoint::ProfileRoute),
            "ADR-076 promised profile_route as its own DecisionPoint"
        );
        assert_ne!(
            DecisionPoint::parse("profile_route"),
            DecisionPoint::parse("grammar_route"),
            "profile_route must not alias grammar_route"
        );
    }

    #[test]
    fn profile_route_codes_roundtrip() {
        assert_eq!(ProfileRoute::from_code(0), Some(ProfileRoute::Structural));
        assert_eq!(ProfileRoute::from_code(1), Some(ProfileRoute::Rich));
        assert!(ProfileRoute::from_code(2).is_none());
        assert_eq!(ProfileRoute::Structural.to_code(), 0);
        assert_eq!(ProfileRoute::Rich.to_code(), 1);
        assert_eq!(ProfileRoute::Structural.as_str(), "structural");
        assert_eq!(ProfileRoute::Rich.as_str(), "rich");
        assert_eq!(
            ProfileRoute::parse("structural"),
            Some(ProfileRoute::Structural)
        );
        assert_eq!(ProfileRoute::parse("rich"), Some(ProfileRoute::Rich));
        assert!(ProfileRoute::parse("grobid_structural").is_none());
        assert!(ProfileRoute::parse("arxiv_pdf_dot").is_none());
    }

    #[test]
    fn profile_route_native_default_is_structural() {
        let cfg = ProfileRouteNative::default();
        assert_eq!(cfg.default, ProfileRoute::Structural);
        let chosen = profile_route_native(&feat(18, 0, 0.85), &cfg);
        assert_eq!(chosen, ProfileRoute::Structural);
        let rich_cfg = ProfileRouteNative {
            default: ProfileRoute::Rich,
        };
        assert_eq!(
            profile_route_native(&feat(18, 0, 0.85), &rich_cfg),
            ProfileRoute::Rich,
            "DATA may name rich; native default must not"
        );
    }

    #[test]
    fn registry_profile_route_default_from_yaml() {
        let yaml = r#"
decision_points:
  grammar_route:
    default_profile: arxiv_pdf_dot
  profile_route:
    native:
      default: structural
"#;
        let reg = PolicyRegistry::from_yaml_str(yaml).unwrap();
        assert_eq!(reg.profile_route_native().default, ProfileRoute::Structural);
        assert_eq!(
            profile_route_native(&feat(1, 0, 1.0), &reg.profile_route_native()),
            ProfileRoute::Structural
        );
    }

    #[test]
    fn policy_decision_profile_serde_roundtrip() {
        let d = PolicyDecision::profile(PolicySource::Native, ProfileRoute::Structural);
        assert_eq!(d.id.decision_point, "profile_route");
        assert_eq!(d.profile, Some(ProfileRoute::Structural));
        assert!(
            d.route.is_none(),
            "profile consult must not stamp grammar_route"
        );
        let json = serde_json::to_string(&d).unwrap();
        let back: PolicyDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
        assert!(
            !json.contains("grammar_route"),
            "profile JSON must not collide with grammar_route field names beyond skip-optional"
        );
    }

    #[test]
    fn grammar_route_codes_roundtrip() {
        for code in 0..3 {
            let r = GrammarRoute::from_code(code).unwrap();
            assert_eq!(r.to_code(), code);
        }
        assert!(GrammarRoute::from_code(9).is_none());
    }

    #[test]
    fn policy_decision_serde_roundtrip() {
        let d = PolicyDecision::grammar(
            PolicySource::Wasm {
                sha256: "abc".into(),
            },
            GrammarRoute::XyCut,
        );
        let json = serde_json::to_string(&d).unwrap();
        let back: PolicyDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
