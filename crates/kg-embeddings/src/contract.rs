//! Port contract suite for Embedder.
//!
//! ADR-051 Contour 4: a single semantic specification run against
//! every implementation (FdApi, future RuVector, test doubles).
//!
//! The suite checks:
//! - embed returns correct dimension
//! - same text → same vector (determinism)
//! - different text → different vector
//! - empty text handling (does not panic)
//! - unicode text handling
//! - embed_batch consistency with embed

use crate::embedder::Embedder;

/// Run the full Embedder contract suite against `embedder`.
///
/// Panics on any contract violation. The caller provides the runtime
/// and a connected embedder instance.
pub async fn embedder_contract_suite<E: Embedder>(embedder: &E) {
    test_embed_returns_correct_dimension(embedder).await;
    test_same_text_same_vector(embedder).await;
    test_different_text_different_vector(embedder).await;
    test_empty_text_does_not_panic(embedder).await;
    test_unicode_text_handled(embedder).await;
    test_batch_consistent_with_single(embedder).await;
}

async fn test_embed_returns_correct_dimension<E: Embedder>(embedder: &E) {
    let vec = embedder.embed("hello world").await.unwrap();
    assert_eq!(
        vec.len(),
        embedder.dimensions(),
        "embed output length must match dimensions()"
    );
}

async fn test_same_text_same_vector<E: Embedder>(embedder: &E) {
    let a = embedder.embed("deterministic input").await.unwrap();
    let b = embedder.embed("deterministic input").await.unwrap();
    assert_eq!(a, b, "same text must produce the same vector (determinism)");
}

async fn test_different_text_different_vector<E: Embedder>(embedder: &E) {
    let a = embedder.embed("first text about cats").await.unwrap();
    let b = embedder.embed("second text about dogs").await.unwrap();
    assert_ne!(a, b, "different text must produce different vectors");
}

async fn test_empty_text_does_not_panic<E: Embedder>(embedder: &E) {
    // Empty text must not panic; it may return an error or a zero vector,
    // but the call itself must be safe.
    let result = embedder.embed("").await;
    assert!(
        result.is_ok(),
        "empty text must not error (got: {:?})",
        result.err()
    );
    let vec = result.unwrap();
    assert_eq!(
        vec.len(),
        embedder.dimensions(),
        "empty text vector must still match dimensions()"
    );
}

async fn test_unicode_text_handled<E: Embedder>(embedder: &E) {
    let texts = ["Привет мир", "你好世界", "🎉🚀✨", "مرحبا بالعالم"];
    for text in &texts {
        let vec = embedder
            .embed(text)
            .await
            .unwrap_or_else(|_| panic!("unicode text '{text}' must not error"));
        assert_eq!(
            vec.len(),
            embedder.dimensions(),
            "unicode text vector must match dimensions()"
        );
    }
}

async fn test_batch_consistent_with_single<E: Embedder>(embedder: &E) {
    let texts = ["alpha", "beta", "gamma"];
    let batch = embedder.embed_batch(&texts).await.unwrap();
    assert_eq!(
        batch.len(),
        texts.len(),
        "embed_batch must return one vector per input"
    );
    for (i, text) in texts.iter().enumerate() {
        let single = embedder.embed(text).await.unwrap();
        assert_eq!(
            batch[i], single,
            "embed_batch[{i}] must match embed('{text}') for determinism"
        );
    }
}
