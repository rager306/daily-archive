//! SUT-free reference producer for the closed parse-policy ablation
//! (M507-4fk6m6/S07/T04).
//!
//! ADR-051 forbids building the oracle out of the same code as the system under
//! test. This module is therefore deliberately disconnected from the production
//! dispatch path: it re-derives the *declared* action of the closed grammar
//! route policy from DATA only — the committed registry's `profile_map`,
//! `default_profile` and `native` thresholds, the documented route-code table,
//! and the frozen case DATA. It never loads a policy module, never consults a
//! decision point, never imports the infrastructure crate that hosts the policy
//! runtime, and never spawns a process. The unit tests in this file scan their
//! own source and fail if any production symbol appears in executable code.
//!
//! Two identities are hash-bound by the ablation artifact:
//!
//! * [`REFERENCE_PRODUCER_ID`] — this declared-DATA recomputation (the
//!   independent arm);
//! * [`PRODUCTION_PRODUCER_ID`] — the production selector that is *measured*,
//!   not trusted (the observed arm).
//!
//! The two arms therefore carry different producer code digests, which is what
//! the shared independence gate in `da-application` requires.

use da_application::{CRITIC_SCHEMA_VERSION, CriticObservation, CriticStatus};
use da_ports::Sha256Digest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

/// Schema identity of the frozen case DATA consumed by the reference producer.
pub const REFERENCE_SCHEMA_VERSION: &str = "parse-policy-ablation-cases.v1";
/// Machine identity of the independent (declared-DATA) producer arm.
pub const REFERENCE_PRODUCER_ID: &str =
    "da-cli::policy_ablation_reference::declared-data-recompute@v1";
/// Machine identity of the measured production producer arm.
/// Assembled in two literals on purpose: the identity must name the production
/// selector it measures, while the source of this module must stay free of any
/// production symbol (see the source scan in the tests below).
pub const PRODUCTION_PRODUCER_ID: &str =
    concat!("da-cli::source_cli::resolve_", "parsed_dispatch@v1");

/// Documented wire table of the closed `GrammarRoute` code space (ADR-074).
/// The WAT fixture header documents the same three codes; out-of-range codes
/// are a typed refusal, never a silent default.
pub const ROUTE_CODE_TABLE: [(i32, &str); 3] = [
    (0, PAGE_OUTLINE_ROUTE),
    (1, XY_CUT_ROUTE),
    (2, GROBID_STRUCTURAL_ROUTE),
];

/// The closed parsed-stage CLI action pair. No policy variant may leave it.
pub const CLOSED_ACTIONS: [&str; 2] = [FALLBACK_ACTION, PAGE_OUTLINE_ACTION];

pub const PAGE_OUTLINE_ROUTE: &str = "page_outline";
pub const XY_CUT_ROUTE: &str = "xy_cut";
pub const GROBID_STRUCTURAL_ROUTE: &str = "grobid_structural";

/// FSM grammar profiles: only the page-outline profile is TOC-driven.
pub const PAGE_OUTLINE_PROFILE: &str = "page_outline";
/// Declared default profile when a route has no `profile_map` entry.
pub const FALLBACK_PROFILE: &str = "arxiv_pdf_dot";

/// Fail-open action for every profile that is not TOC-driven.
pub const FALLBACK_ACTION: &str = "parse-pdf";
/// Action taken only when a page-outline profile meets an observed TOC cache.
pub const PAGE_OUTLINE_ACTION: &str = "parse-pdf-toc";

/// Documented DATA defaults of the native grammar route (mirrors the
/// `native:` block of `data/policies/registry.yaml`).
pub const DEFAULT_PAGE_OUTLINE_MIN_PAGES: u32 = 50;
pub const DEFAULT_PAGE_OUTLINE_MIN_BOOKMARKS: u32 = 1;
pub const DEFAULT_GROBID_MAX_TEXT_QUALITY: f32 = 0.4;

/// Typed refusals of the reference producer. Every deviation from the closed
/// DATA contract is a named error; there is no silent default.
#[derive(Debug, Clone, PartialEq)]
pub enum ReferenceError {
    UnsupportedSchemaVersion(String),
    EmptyCases,
    DuplicateCaseId(String),
    DuplicatePaperId(String),
    UnsafePaperId(String),
    InvalidSplit(String),
    MissingActionInSplit { split: String, action: String },
    MalformedCase(String),
    MalformedRegistry(String),
    MissingDeclaredProfile,
    UnknownAction(String),
    UnknownRouteCode(i32),
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion(found) => {
                write!(f, "unsupported case schema version `{found}`")
            }
            Self::EmptyCases => write!(f, "case set is empty"),
            Self::DuplicateCaseId(id) => write!(f, "duplicate case id `{id}`"),
            Self::DuplicatePaperId(id) => write!(f, "duplicate paper id `{id}`"),
            Self::UnsafePaperId(id) => write!(f, "unsafe paper id `{id}`"),
            Self::InvalidSplit(split) => write!(f, "invalid or empty split `{split}`"),
            Self::MissingActionInSplit { split, action } => {
                write!(f, "split `{split}` never exercises action `{action}`")
            }
            Self::MalformedCase(detail) => write!(f, "malformed case DATA: {detail}"),
            Self::MalformedRegistry(detail) => write!(f, "malformed policy registry: {detail}"),
            Self::MissingDeclaredProfile => {
                write!(f, "registry declares no profile for the route")
            }
            Self::UnknownAction(action) => write!(f, "action `{action}` is outside the closed set"),
            Self::UnknownRouteCode(code) => {
                write!(f, "route code {code} is outside the closed set")
            }
        }
    }
}

impl std::error::Error for ReferenceError {}

/// Article features as declared by one frozen case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseFeatures {
    pub pages: u32,
    pub bookmarks: u32,
    pub quality: f32,
}

impl CaseFeatures {
    /// Same milli-quantisation the policy wire uses, applied to the declared
    /// quality only (no policy runtime participates).
    pub fn quality_milli(&self) -> i32 {
        (self.quality.clamp(0.0, 1.0) * 1000.0).round() as i32
    }
}

/// Frozen declared expectations of one case, one per arm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedActions {
    pub baseline: String,
    pub candidate: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseSplit {
    Train,
    Validation,
}

impl CaseSplit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "validation",
        }
    }
}

/// One frozen ordered case: identity, features, TOC observation and the
/// declared action of each arm. No payloads, no article text, no PII.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyAblationCase {
    pub case_id: String,
    pub split: CaseSplit,
    pub paper_id: String,
    pub features: CaseFeatures,
    /// Whether the parsed-stage TOC observation cache exists for this case.
    pub toc_cache: bool,
    pub expect: ExpectedActions,
}

/// The frozen case DATA document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyAblationCases {
    pub schema_version: String,
    pub cases: Vec<PolicyAblationCase>,
}

/// Case identity used as the split membership key: everything that decides the
/// expected action, and nothing that depends on the runner.
#[derive(Serialize)]
struct CaseIdentity<'a> {
    case_id: &'a str,
    paper_id: &'a str,
    features: &'a CaseFeatures,
    toc_cache: bool,
    expect: &'a ExpectedActions,
}

/// Content digest of arbitrary bytes (shared by every reference identity).
pub fn content_digest(bytes: &[u8]) -> Sha256Digest {
    Sha256Digest::new(hex::encode(Sha256::digest(bytes))).expect("sha256 hex is a valid digest")
}

/// Deterministic machine identity of a producer arm.
pub fn producer_identity_digest(producer_id: &str) -> Sha256Digest {
    content_digest(producer_id.as_bytes())
}

/// Frozen case identity: the value `SplitIdentity` partitions.
pub fn case_identity_digest(case: &PolicyAblationCase) -> Sha256Digest {
    let identity = CaseIdentity {
        case_id: &case.case_id,
        paper_id: &case.paper_id,
        features: &case.features,
        toc_cache: case.toc_cache,
        expect: &case.expect,
    };
    content_digest(&serde_json::to_vec(&identity).expect("case identity serializes"))
}

/// Frozen corpus identity shared by every case and every producer input.
pub fn corpus_digest(cases: &[PolicyAblationCase]) -> Sha256Digest {
    content_digest(&serde_json::to_vec(cases).expect("cases serialize"))
}

/// The closed route name of a documented wire code, or `None` outside the set.
pub fn route_for_code(code: i32) -> Option<&'static str> {
    ROUTE_CODE_TABLE
        .iter()
        .find(|(wire, _)| *wire == code)
        .map(|(_, route)| *route)
}

/// The documented profile to action mapping. Only a page-outline profile that
/// meets an observed TOC cache selects the TOC action; anything else keeps the
/// fail-open parse action. No other action is reachable.
pub fn closed_action_for_profile(profile: &str, toc_present: bool) -> &'static str {
    if profile == PAGE_OUTLINE_PROFILE && toc_present {
        PAGE_OUTLINE_ACTION
    } else {
        FALLBACK_ACTION
    }
}

/// Whether an action is inside the closed parsed-stage action pair.
pub fn is_closed_action(action: &str) -> bool {
    CLOSED_ACTIONS.contains(&action)
}

/// The DECLARED grammar policy of one variant, derived from registry DATA only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeclaredGrammarPolicy {
    profile_map: BTreeMap<String, String>,
    default_profile: Option<String>,
    page_outline_min_pages: u32,
    page_outline_min_bookmarks: u32,
    grobid_max_text_quality: f32,
}

#[derive(Deserialize)]
struct RegistryDoc {
    #[serde(default)]
    decision_points: BTreeMap<String, RegistryPoint>,
}

#[derive(Deserialize)]
struct RegistryPoint {
    #[serde(default)]
    default_profile: Option<String>,
    #[serde(default)]
    profile_map: BTreeMap<String, String>,
    #[serde(default)]
    native: RegistryNative,
}

#[derive(Deserialize, Default)]
struct RegistryNative {
    #[serde(default)]
    page_outline_min_pages: Option<u32>,
    #[serde(default)]
    page_outline_min_bookmarks: Option<u32>,
    #[serde(default)]
    grobid_max_text_quality: Option<f32>,
}

impl DeclaredGrammarPolicy {
    /// Read the DECLARED grammar route policy out of the registry DATA.
    ///
    /// Only the declarations that decide a dispatch are consumed:
    /// `grammar_route.profile_map`, `grammar_route.default_profile` and the
    /// `grammar_route.native` thresholds. A registry without a `grammar_route`
    /// point is a typed refusal.
    pub fn from_registry_yaml(yaml: &str) -> Result<Self, ReferenceError> {
        let doc: RegistryDoc = serde_yaml::from_str(yaml)
            .map_err(|e| ReferenceError::MalformedRegistry(e.to_string()))?;
        let point = doc
            .decision_points
            .get("grammar_route")
            .ok_or_else(|| ReferenceError::MalformedRegistry("no grammar_route point".into()))?;
        Ok(Self {
            profile_map: point.profile_map.clone(),
            default_profile: point.default_profile.clone(),
            page_outline_min_pages: point
                .native
                .page_outline_min_pages
                .unwrap_or(DEFAULT_PAGE_OUTLINE_MIN_PAGES),
            page_outline_min_bookmarks: point
                .native
                .page_outline_min_bookmarks
                .unwrap_or(DEFAULT_PAGE_OUTLINE_MIN_BOOKMARKS),
            grobid_max_text_quality: point
                .native
                .grobid_max_text_quality
                .unwrap_or(DEFAULT_GROBID_MAX_TEXT_QUALITY),
        })
    }

    pub fn page_outline_min_pages(&self) -> u32 {
        self.page_outline_min_pages
    }

    pub fn page_outline_min_bookmarks(&self) -> u32 {
        self.page_outline_min_bookmarks
    }

    pub fn grobid_max_text_quality(&self) -> f32 {
        self.grobid_max_text_quality
    }

    /// Threshold rule of the DATA-declared baseline policy: page outline when
    /// the article is long enough AND carries bookmarks, otherwise a low text
    /// layer means the structural route, otherwise the XY-cut route.
    pub fn declared_route_for_features(&self, features: &CaseFeatures) -> &'static str {
        if features.pages >= self.page_outline_min_pages
            && features.bookmarks >= self.page_outline_min_bookmarks
        {
            return PAGE_OUTLINE_ROUTE;
        }
        let grobid_milli = (self.grobid_max_text_quality.clamp(0.0, 1.0) * 1000.0).round() as i32;
        if features.quality_milli() < grobid_milli {
            return GROBID_STRUCTURAL_ROUTE;
        }
        XY_CUT_ROUTE
    }

    /// Declared action of a route: route -> profile via DATA `profile_map`
    /// (falling back to the declared `default_profile`), profile -> action via
    /// the documented closed mapping.
    pub fn declared_action(
        &self,
        route: &str,
        toc_present: bool,
    ) -> Result<&'static str, ReferenceError> {
        let profile = self
            .profile_map
            .get(route)
            .or(self.default_profile.as_ref())
            .ok_or(ReferenceError::MissingDeclaredProfile)?;
        Ok(closed_action_for_profile(profile, toc_present))
    }

    /// Declared action of a documented wire code; an out-of-range code is a
    /// typed refusal rather than a silent default.
    pub fn declared_action_for_code(
        &self,
        code: i32,
        toc_present: bool,
    ) -> Result<&'static str, ReferenceError> {
        let route = route_for_code(code).ok_or(ReferenceError::UnknownRouteCode(code))?;
        self.declared_action(route, toc_present)
    }
}

/// Parse and validate the frozen case DATA.
///
/// Fail-closed on: unsupported schema, empty set, duplicate case or paper
/// identity, a paper identity that is not a safe single path component,
/// an unknown declared action, a non-finite/out-of-range quality, an empty
/// split, and a split that never exercises both closed actions.
pub fn parse_cases(json: &str) -> Result<Vec<PolicyAblationCase>, ReferenceError> {
    let doc: PolicyAblationCases =
        serde_json::from_str(json).map_err(|e| ReferenceError::MalformedCase(e.to_string()))?;
    if doc.schema_version != REFERENCE_SCHEMA_VERSION {
        return Err(ReferenceError::UnsupportedSchemaVersion(doc.schema_version));
    }
    if doc.cases.is_empty() {
        return Err(ReferenceError::EmptyCases);
    }
    let mut seen_ids: BTreeMap<&str, ()> = BTreeMap::new();
    let mut seen_papers: BTreeMap<&str, ()> = BTreeMap::new();
    for case in &doc.cases {
        if seen_ids.insert(case.case_id.as_str(), ()).is_some() {
            return Err(ReferenceError::DuplicateCaseId(case.case_id.clone()));
        }
        if !is_safe_paper_id(&case.paper_id) {
            return Err(ReferenceError::UnsafePaperId(case.paper_id.clone()));
        }
        if seen_papers.insert(case.paper_id.as_str(), ()).is_some() {
            return Err(ReferenceError::DuplicatePaperId(case.paper_id.clone()));
        }
        if !case.features.quality.is_finite() || !(0.0..=1.0).contains(&case.features.quality) {
            return Err(ReferenceError::MalformedCase(format!(
                "case `{}` quality is not a fraction",
                case.case_id
            )));
        }
        for action in [&case.expect.baseline, &case.expect.candidate] {
            if !is_closed_action(action) {
                return Err(ReferenceError::UnknownAction(action.clone()));
            }
        }
    }
    for split in [CaseSplit::Train, CaseSplit::Validation] {
        let members: Vec<&PolicyAblationCase> = doc
            .cases
            .iter()
            .filter(|case| case.split == split)
            .collect();
        if members.is_empty() {
            return Err(ReferenceError::InvalidSplit(split.as_str().into()));
        }
        for action in CLOSED_ACTIONS {
            let exercised = members
                .iter()
                .any(|case| case.expect.baseline == action || case.expect.candidate == action);
            if !exercised {
                return Err(ReferenceError::MissingActionInSplit {
                    split: split.as_str().into(),
                    action: action.into(),
                });
            }
        }
    }
    Ok(doc.cases)
}

/// Paper identity must be a single safe path component: it is the cache file
/// stem, so separators and traversal segments are refused before any join.
fn is_safe_paper_id(paper_id: &str) -> bool {
    !paper_id.is_empty()
        && paper_id != "."
        && paper_id != ".."
        && paper_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// One redacted critic row for one arm.
///
/// `measured` is what the arm actually selected: `None` means the arm failed
/// closed. A failed-closed case STAYS in the denominator with a zero metric and
/// a typed status, so dropping a failure can never raise the score.
pub fn observation_row(
    case: &PolicyAblationCase,
    corpus: &Sha256Digest,
    order: u32,
    producer: &str,
    measured: Option<&str>,
    expected: &str,
) -> Result<CriticObservation, ReferenceError> {
    if !is_closed_action(expected) {
        return Err(ReferenceError::UnknownAction(expected.to_string()));
    }
    if let Some(action) = measured
        && !is_closed_action(action)
    {
        return Err(ReferenceError::UnknownAction(action.to_string()));
    }
    let (metric_one, status) = match measured {
        Some(action) if action == expected => (1.0, CriticStatus::Ok),
        Some(_) => (0.0, CriticStatus::Ok),
        None => (0.0, CriticStatus::Timeout),
    };
    let metric_two = match measured {
        Some(PAGE_OUTLINE_ACTION) => 1.0,
        _ => 0.0,
    };
    Ok(CriticObservation {
        schema_version: CRITIC_SCHEMA_VERSION,
        case_id: case.case_id.clone(),
        corpus: corpus.clone(),
        order,
        producer: producer.into(),
        metric_one,
        metric_two,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The production symbols this module must never reach for. Every token is
    /// assembled at compile time so the scan target (this very file) does not
    /// contain a single one of them contiguously.
    fn sut_tokens() -> Vec<String> {
        vec![
            concat!("Wasm", "Policy", "Host").into(),
            concat!("Fsm", "Policy", "Port").into(),
            concat!("resolve_parse", "_grammar").into(),
            concat!("resolve_parsed", "_dispatch").into(),
            concat!("consult_parse", "_grammar").into(),
            concat!("da", "_adapters").into(),
            concat!("da", "-adapters").into(),
            concat!("std::", "process").into(),
            concat!("Command", "::new").into(),
        ]
    }

    /// Scan a Rust source for production tokens, ignoring comment lines.
    fn scan_for_sut_tokens(source: &str) -> Vec<String> {
        let code: String = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !(trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                    || trimmed.starts_with('*'))
            })
            .collect::<Vec<_>>()
            .join("\n");
        sut_tokens()
            .into_iter()
            .filter(|token| code.contains(token.as_str()))
            .collect()
    }

    /// `file!()` is workspace-relative under cargo; resolve it from the
    /// manifest directory when the test CWD is the package root.
    fn self_source() -> String {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let candidates = [
            std::path::PathBuf::from(file!()),
            manifest.join(file!()),
            manifest.join("src/policy_ablation_reference.rs"),
        ];
        for path in candidates {
            if let Ok(text) = std::fs::read_to_string(&path) {
                assert!(
                    text.contains(REFERENCE_PRODUCER_ID),
                    "scanned an unrelated file: {}",
                    path.display()
                );
                return text;
            }
        }
        panic!("cannot locate {}", file!());
    }

    fn registry_yaml() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/policies/registry.yaml");
        std::fs::read_to_string(&path).expect("committed registry must be readable")
    }

    fn cases_json() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/policy_ablation_cases.json");
        std::fs::read_to_string(&path).expect("frozen case DATA must be readable")
    }

    fn cases() -> Vec<PolicyAblationCase> {
        parse_cases(&cases_json()).expect("frozen case DATA must validate")
    }

    // ───────────────── independence of the oracle (ADR-051) ─────────────────

    #[test]
    fn reference_producer_source_is_free_of_sut_tokens() {
        let source = self_source();
        let hits = scan_for_sut_tokens(&source);
        assert!(
            hits.is_empty(),
            "the reference producer must not touch the system under test: {hits:?}"
        );
    }

    #[test]
    fn scanner_actually_detects_an_injected_token() {
        let poisoned = format!(
            "fn oracle() {{ let _ = {}::new(); }}\n",
            concat!("Wasm", "Policy", "Host")
        );
        assert_eq!(
            scan_for_sut_tokens(&poisoned),
            vec![concat!("Wasm", "Policy", "Host").to_string()],
            "a production token in executable code must be flagged"
        );
        let commented = format!(
            "// {} is banned here\n",
            concat!("resolve_parsed", "_dispatch")
        );
        assert!(
            scan_for_sut_tokens(&commented).is_empty(),
            "prose in comments is not an oracle dependency"
        );
        let process = format!("let _ = {};\n", concat!("std::", "process", "::exit(0)"));
        assert!(
            !scan_for_sut_tokens(&process).is_empty(),
            "spawning a process is not an SUT-free oracle"
        );
    }

    // ───────────────── the closed route/action wire ─────────────────

    #[test]
    fn route_code_table_is_the_documented_closed_set() {
        assert_eq!(route_for_code(0), Some(PAGE_OUTLINE_ROUTE));
        assert_eq!(route_for_code(1), Some(XY_CUT_ROUTE));
        assert_eq!(route_for_code(2), Some(GROBID_STRUCTURAL_ROUTE));
        for code in [3, 7, -1, i32::MAX] {
            assert_eq!(route_for_code(code), None, "code {code} must be refused");
        }
        assert_eq!(ROUTE_CODE_TABLE.len(), 3);
        assert_eq!(CLOSED_ACTIONS.len(), 2);
        assert!(is_closed_action(FALLBACK_ACTION));
        assert!(is_closed_action(PAGE_OUTLINE_ACTION));
        assert!(!is_closed_action("native-fulltext-compose"));
    }

    #[test]
    fn unknown_code_and_action_fail_closed() {
        let policy = DeclaredGrammarPolicy::from_registry_yaml(&registry_yaml()).unwrap();
        assert_eq!(
            policy.declared_action_for_code(3, true),
            Err(ReferenceError::UnknownRouteCode(3))
        );
        let case = cases().remove(0);
        let corpus = corpus_digest(&cases());
        assert_eq!(
            observation_row(
                &case,
                &corpus,
                0,
                "arm",
                Some("invented-action"),
                "parse-pdf"
            ),
            Err(ReferenceError::UnknownAction("invented-action".into()))
        );
        assert_eq!(
            observation_row(
                &case,
                &corpus,
                0,
                "arm",
                Some("parse-pdf"),
                "invented-action"
            ),
            Err(ReferenceError::UnknownAction("invented-action".into()))
        );
    }

    #[test]
    fn fail_closed_dispatch_stays_in_the_denominator_with_zero_metrics() {
        let case = cases().remove(0);
        let corpus = corpus_digest(&cases());
        let row = observation_row(&case, &corpus, 0, "measured-arm", None, FALLBACK_ACTION)
            .expect("a failed-closed arm is typed, not fatal");
        assert_eq!(row.status, CriticStatus::Timeout);
        assert_eq!(row.metric_one, 0.0);
        assert_eq!(row.metric_two, 0.0);
        assert_eq!(row.case_id, case.case_id);
        assert_eq!(row.corpus, corpus);
        assert_eq!(row.order, 0);
    }

    #[test]
    fn observation_rows_score_agreement_and_non_fallback_rate() {
        let case = cases().remove(0);
        let corpus = corpus_digest(&cases());
        let agree = observation_row(
            &case,
            &corpus,
            0,
            "arm",
            Some(FALLBACK_ACTION),
            FALLBACK_ACTION,
        )
        .expect("closed actions are accepted");
        assert_eq!(
            (agree.metric_one, agree.metric_two, agree.status),
            (1.0, 0.0, CriticStatus::Ok)
        );
        let toc = observation_row(
            &case,
            &corpus,
            0,
            "arm",
            Some(PAGE_OUTLINE_ACTION),
            PAGE_OUTLINE_ACTION,
        )
        .expect("closed actions are accepted");
        assert_eq!((toc.metric_one, toc.metric_two), (1.0, 1.0));
        let disagreed = observation_row(
            &case,
            &corpus,
            0,
            "arm",
            Some(PAGE_OUTLINE_ACTION),
            FALLBACK_ACTION,
        )
        .expect("closed actions are accepted");
        assert_eq!((disagreed.metric_one, disagreed.metric_two), (0.0, 1.0));
    }

    // ───────────────── DATA-derived declared policy ─────────────────

    #[test]
    fn declared_policy_reads_thresholds_and_profile_map_from_data() {
        let policy = DeclaredGrammarPolicy::from_registry_yaml(&registry_yaml()).unwrap();
        assert_eq!(policy.page_outline_min_pages(), 50);
        assert_eq!(policy.page_outline_min_bookmarks(), 1);
        assert_eq!(policy.grobid_max_text_quality(), 0.4);
        assert_eq!(
            policy.declared_action(XY_CUT_ROUTE, false),
            Ok(FALLBACK_ACTION)
        );
        assert_eq!(
            policy.declared_action(PAGE_OUTLINE_ROUTE, true),
            Ok(PAGE_OUTLINE_ACTION)
        );
        assert_eq!(
            policy.declared_action(PAGE_OUTLINE_ROUTE, false),
            Ok(FALLBACK_ACTION),
            "a page-outline profile without an observed TOC must keep the fail-open action"
        );
        assert_eq!(
            policy.declared_action("invented_route", false),
            Ok(FALLBACK_ACTION),
            "an unknown route falls back to the DATA default_profile"
        );
    }

    #[test]
    fn declared_route_follows_the_data_thresholds() {
        let policy = DeclaredGrammarPolicy::from_registry_yaml(&registry_yaml()).unwrap();
        let features = |pages, bookmarks, quality| CaseFeatures {
            pages,
            bookmarks,
            quality,
        };
        assert_eq!(
            policy.declared_route_for_features(&features(120, 6, 0.9)),
            PAGE_OUTLINE_ROUTE
        );
        assert_eq!(
            policy.declared_route_for_features(&features(18, 0, 0.85)),
            XY_CUT_ROUTE
        );
        assert_eq!(
            policy.declared_route_for_features(&features(18, 0, 0.2)),
            GROBID_STRUCTURAL_ROUTE
        );
        assert_eq!(
            policy.declared_route_for_features(&features(50, 1, 0.1)),
            PAGE_OUTLINE_ROUTE,
            "the thresholds are inclusive on the long-monograph side"
        );
    }

    /// The frozen case DATA must agree with the DATA-declared baseline policy:
    /// if the expectations and the declarations diverge, this oracle is not an
    /// oracle for the baseline arm any more.
    #[test]
    fn frozen_case_expectations_match_the_declared_baseline_policy() {
        let policy = DeclaredGrammarPolicy::from_registry_yaml(&registry_yaml()).unwrap();
        for case in cases() {
            let route = policy.declared_route_for_features(&case.features);
            let action = policy
                .declared_action(route, case.toc_cache)
                .expect("the committed DATA declares a profile for every closed route");
            assert_eq!(
                action, case.expect.baseline,
                "case `{}` declares `{}` but the DATA-declared baseline yields `{action}`",
                case.case_id, case.expect.baseline
            );
        }
    }

    #[test]
    fn registry_without_grammar_route_is_a_typed_refusal() {
        assert!(matches!(
            DeclaredGrammarPolicy::from_registry_yaml(
                "decision_points:\n  profile_route:\n    native: {}\n"
            ),
            Err(ReferenceError::MalformedRegistry(_))
        ));
        assert!(matches!(
            DeclaredGrammarPolicy::from_registry_yaml("decision_points: [not-a-map\n"),
            Err(ReferenceError::MalformedRegistry(_))
        ));
    }

    #[test]
    fn registry_without_any_profile_declaration_refuses_to_guess() {
        let yaml = "decision_points:\n  grammar_route:\n    native: {}\n";
        let policy = DeclaredGrammarPolicy::from_registry_yaml(yaml).unwrap();
        assert_eq!(
            policy.declared_action(PAGE_OUTLINE_ROUTE, true),
            Err(ReferenceError::MissingDeclaredProfile)
        );
    }

    // ───────────────── frozen case DATA validation ─────────────────

    #[test]
    fn frozen_case_data_is_ordered_unique_and_split_balanced() {
        let frozen = cases();
        let train = frozen
            .iter()
            .filter(|c| c.split == CaseSplit::Train)
            .count();
        let validation = frozen
            .iter()
            .filter(|c| c.split == CaseSplit::Validation)
            .count();
        assert_eq!(train + validation, frozen.len());
        assert!(train > 0 && validation > 0);
        let digests: std::collections::BTreeSet<Sha256Digest> =
            frozen.iter().map(case_identity_digest).collect();
        assert_eq!(
            digests.len(),
            frozen.len(),
            "case identities must be unique to serve as split members"
        );
    }

    #[test]
    fn unsafe_or_duplicated_case_data_is_refused() {
        let good = cases().remove(0);
        let render = |case: &PolicyAblationCase| {
            serde_json::to_string(&PolicyAblationCases {
                schema_version: REFERENCE_SCHEMA_VERSION.into(),
                cases: vec![case.clone()],
            })
            .unwrap()
        };

        let mut traversal = good.clone();
        traversal.paper_id = "../../etc/passwd".into();
        assert_eq!(
            parse_cases(&render(&traversal)),
            Err(ReferenceError::UnsafePaperId("../../etc/passwd".into()))
        );

        let mut separator = good.clone();
        separator.paper_id = "nested/id".into();
        assert!(matches!(
            parse_cases(&render(&separator)),
            Err(ReferenceError::UnsafePaperId(_))
        ));

        let duplicate = serde_json::to_string(&PolicyAblationCases {
            schema_version: REFERENCE_SCHEMA_VERSION.into(),
            cases: vec![good.clone(), good.clone()],
        })
        .unwrap();
        assert_eq!(
            parse_cases(&duplicate),
            Err(ReferenceError::DuplicateCaseId(good.case_id.clone()))
        );

        let mut bad_action = good.clone();
        bad_action.expect.candidate = "rm -rf".into();
        assert_eq!(
            parse_cases(&render(&bad_action)),
            Err(ReferenceError::UnknownAction("rm -rf".into()))
        );

        let mut bad_quality = good.clone();
        bad_quality.features.quality = f32::NAN;
        assert!(matches!(
            parse_cases(&render(&bad_quality)),
            Err(ReferenceError::MalformedCase(_))
        ));

        let mut bad_split = good.clone();
        bad_split.split = CaseSplit::Validation;
        assert_eq!(
            parse_cases(&render(&bad_split)),
            Err(ReferenceError::InvalidSplit("train".into())),
            "a split with no member is refused"
        );

        // A split that never exercises one of the two closed actions is not a
        // qualified same-n set.
        let plain = cases()
            .into_iter()
            .find(|case| {
                case.expect.baseline == FALLBACK_ACTION && case.expect.candidate == FALLBACK_ACTION
            })
            .expect("the frozen set must contain a fail-open-only case");
        let mut second = plain.clone();
        second.case_id = "train-plain-second".into();
        second.paper_id = "5070.30001".into();
        let unbalanced = serde_json::to_string(&PolicyAblationCases {
            schema_version: REFERENCE_SCHEMA_VERSION.into(),
            cases: vec![plain, second],
        })
        .unwrap();
        assert_eq!(
            parse_cases(&unbalanced),
            Err(ReferenceError::MissingActionInSplit {
                split: "train".into(),
                action: PAGE_OUTLINE_ACTION.into(),
            })
        );

        assert_eq!(
            parse_cases("{\"schema_version\":\"other\",\"cases\":[]}"),
            Err(ReferenceError::UnsupportedSchemaVersion("other".into()))
        );
        assert!(matches!(
            parse_cases("{\"schema_version\":\"parse-policy-ablation-cases.v1\",\"cases\":[]}"),
            Err(ReferenceError::EmptyCases)
        ));
        assert!(matches!(
            parse_cases("not json"),
            Err(ReferenceError::MalformedCase(_))
        ));
    }

    // ───────────────── determinism ─────────────────

    #[test]
    fn identities_are_deterministic_and_sensitive() {
        let first = cases();
        let again = cases();
        assert_eq!(corpus_digest(&first), corpus_digest(&again));
        assert_eq!(
            case_identity_digest(&first[0]),
            case_identity_digest(&again[0])
        );
        assert_ne!(
            case_identity_digest(&first[0]),
            case_identity_digest(&first[1])
        );

        let mut changed = first.clone();
        changed[0].expect.candidate = if changed[0].expect.candidate == FALLBACK_ACTION {
            PAGE_OUTLINE_ACTION.into()
        } else {
            FALLBACK_ACTION.into()
        };
        assert_ne!(
            case_identity_digest(&first[0]),
            case_identity_digest(&changed[0]),
            "a changed declaration must change the case identity"
        );
        assert_ne!(corpus_digest(&first), corpus_digest(&changed));
        assert_ne!(
            producer_identity_digest(REFERENCE_PRODUCER_ID),
            producer_identity_digest(PRODUCTION_PRODUCER_ID)
        );
    }
}
