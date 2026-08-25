//! WASM FSM policy host (ADR-074 Layer 3).
//!
//! Loads a versioned `.wat`/`.wasm` artifact, instantiates it in wasmi,
//! and consults the exported `route(pages, bookmarks, qmilli) -> i32`.
//! Hot-swap = rewrite the file and call [`WasmPolicyHost::reload`].
//! LoRA weights are D136-gated and are not loaded here.

use std::path::{Path, PathBuf};

use da_domain::fsm_policy::{
    ArticleFeatures, DecisionPoint, GrammarRoute, PolicyDecision, PolicySource,
};
use da_ports::fsm_policy::{FsmPolicyError, FsmPolicyPort, FsmPolicyResult};
use sha2::{Digest, Sha256};
use wasmi::{Engine, Linker, Module, Store, TypedFunc};

struct LiveModule {
    store: Store<()>,
    route: TypedFunc<(i32, i32, i32), i32>,
    sha256: String,
}

pub struct WasmPolicyHost {
    path: PathBuf,
    point: DecisionPoint,
    engine: Engine,
    live: Option<LiveModule>,
}

impl std::fmt::Debug for WasmPolicyHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmPolicyHost")
            .field("path", &self.path)
            .field("point", &self.point.as_str())
            .field("hash", &self.policy_hash())
            .finish()
    }
}

impl WasmPolicyHost {
    pub fn from_path(path: impl AsRef<Path>, point: DecisionPoint) -> FsmPolicyResult<Self> {
        let path = path.as_ref().to_path_buf();
        let engine = Engine::default();
        let mut host = Self {
            path,
            point,
            engine,
            live: None,
        };
        host.reload()?;
        Ok(host)
    }

    fn compile_bytes(path: &Path) -> FsmPolicyResult<Vec<u8>> {
        let raw = std::fs::read(path).map_err(|e| FsmPolicyError::Io(e.to_string()))?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "wat" || looks_like_wat(&raw) {
            wat::parse_bytes(&raw)
                .map(|cow| cow.into_owned())
                .map_err(|e| FsmPolicyError::Load(e.to_string()))
        } else {
            Ok(raw)
        }
    }

    fn instantiate(engine: &Engine, wasm: &[u8], sha256: String) -> FsmPolicyResult<LiveModule> {
        let module = Module::new(engine, wasm).map_err(|e| FsmPolicyError::Load(e.to_string()))?;
        let mut store = Store::new(engine, ());
        let linker = Linker::new(engine);
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|e| FsmPolicyError::Instantiate(e.to_string()))?;
        let route = instance
            .get_typed_func::<(i32, i32, i32), i32>(&store, "route")
            .map_err(|e| FsmPolicyError::Instantiate(format!("export route: {e}")))?;
        Ok(LiveModule {
            store,
            route,
            sha256,
        })
    }
}

fn looks_like_wat(raw: &[u8]) -> bool {
    let s = std::str::from_utf8(raw).unwrap_or("");
    s.trim_start().starts_with("(module")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

impl WasmPolicyHost {
    pub fn reload(&mut self) -> FsmPolicyResult<String> {
        let wasm = Self::compile_bytes(&self.path)?;
        let sha = sha256_hex(&wasm);
        self.live = Some(Self::instantiate(&self.engine, &wasm, sha.clone())?);
        Ok(sha)
    }

    pub fn policy_hash(&self) -> String {
        self.live
            .as_ref()
            .map(|l| l.sha256.clone())
            .unwrap_or_else(|| "unloaded".into())
    }

    pub fn consult_mut(
        &mut self,
        point: DecisionPoint,
        features: &ArticleFeatures,
    ) -> FsmPolicyResult<PolicyDecision> {
        if point != self.point {
            return Err(FsmPolicyError::UnknownPoint(point.as_str().into()));
        }
        let live = self
            .live
            .as_mut()
            .ok_or_else(|| FsmPolicyError::Load("no module loaded".into()))?;
        let code = live
            .route
            .call(
                &mut live.store,
                (
                    features.page_count as i32,
                    features.toc_bookmarks as i32,
                    features.quality_milli(),
                ),
            )
            .map_err(|e| FsmPolicyError::Trap(e.to_string()))?;
        let route = GrammarRoute::from_code(code).ok_or(FsmPolicyError::InvalidCode(code))?;
        Ok(PolicyDecision::grammar(
            PolicySource::Wasm {
                sha256: live.sha256.clone(),
            },
            route,
        ))
    }
}

/// Interior-mutable host so [`FsmPolicyPort::consult`] can stay `&self`.
pub struct SharedWasmPolicyHost {
    inner: std::sync::Mutex<WasmPolicyHost>,
}

impl SharedWasmPolicyHost {
    pub fn from_path(path: impl AsRef<Path>, point: DecisionPoint) -> FsmPolicyResult<Self> {
        Ok(Self {
            inner: std::sync::Mutex::new(WasmPolicyHost::from_path(path, point)?),
        })
    }
}

impl FsmPolicyPort for SharedWasmPolicyHost {
    fn consult(
        &self,
        point: DecisionPoint,
        features: &ArticleFeatures,
    ) -> FsmPolicyResult<PolicyDecision> {
        let mut g = self
            .inner
            .lock()
            .map_err(|e| FsmPolicyError::Trap(e.to_string()))?;
        g.consult_mut(point, features)
    }

    fn reload(&mut self) -> FsmPolicyResult<String> {
        self.inner
            .get_mut()
            .map_err(|e| FsmPolicyError::Trap(e.to_string()))?
            .reload()
    }

    fn policy_hash(&self) -> String {
        self.inner
            .lock()
            .map(|g| g.policy_hash())
            .unwrap_or_else(|_| "lock-poisoned".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/policies/fixtures")
            .join(name)
    }

    fn feat(pages: u32, bookmarks: u32, quality: f32) -> ArticleFeatures {
        ArticleFeatures {
            page_count: pages,
            toc_bookmarks: bookmarks,
            text_layer_quality: quality,
            domain: "computer_science".into(),
        }
    }

    #[test]
    fn wat_fixture_routes_like_native() {
        let mut host =
            WasmPolicyHost::from_path(fixture("grammar_route.wat"), DecisionPoint::GrammarRoute)
                .expect("load wat");
        let d = host
            .consult_mut(DecisionPoint::GrammarRoute, &feat(700, 40, 0.9))
            .unwrap();
        assert_eq!(d.route, Some(GrammarRoute::PageOutline));
        assert!(matches!(d.id.source, PolicySource::Wasm { .. }));
        let d2 = host
            .consult_mut(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85))
            .unwrap();
        assert_eq!(d2.route, Some(GrammarRoute::XyCut));
        let d3 = host
            .consult_mut(DecisionPoint::GrammarRoute, &feat(12, 0, 0.1))
            .unwrap();
        assert_eq!(d3.route, Some(GrammarRoute::GrobidStructural));
    }

    #[test]
    fn hot_swap_changes_hash_and_decision() {
        let src = std::fs::read_to_string(fixture("grammar_route.wat")).unwrap();
        let alt = std::fs::read_to_string(fixture("grammar_route_always_grobid.wat")).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live.wat");
        std::fs::write(&path, &src).unwrap();

        let mut host = SharedWasmPolicyHost::from_path(&path, DecisionPoint::GrammarRoute).unwrap();
        let before = host.policy_hash();
        let d0 = host
            .consult(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85))
            .unwrap();
        assert_eq!(d0.route, Some(GrammarRoute::XyCut));

        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(alt.as_bytes()).unwrap();
        }
        let after = host.reload().unwrap();
        assert_ne!(before, after, "content hash must change on hot-swap");
        let d1 = host
            .consult(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85))
            .unwrap();
        assert_eq!(d1.route, Some(GrammarRoute::GrobidStructural));
        match d1.id.source {
            PolicySource::Wasm { sha256 } => assert_eq!(sha256, after),
            other => panic!("expected wasm source, got {other:?}"),
        }
    }

    #[test]
    fn missing_file_is_load_error() {
        let err = WasmPolicyHost::from_path("/no/such/policy.wat", DecisionPoint::GrammarRoute)
            .unwrap_err();
        assert!(matches!(err, FsmPolicyError::Io(_) | FsmPolicyError::Load(_)));
    }
}
