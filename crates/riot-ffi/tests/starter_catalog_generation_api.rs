//! Starter-catalog generation restore seams, exercised through the PUBLIC FFI
//! surface rather than the crate-internal `mobile_state` functions.
//!
//! The generation marker is persisted by the native host and handed back on
//! restore. `mobile_state` already has inline tests for the validation and
//! retention rules; these cover the exported `mobile_api` wrappers the native
//! shells actually call, so a wrapper that forgets to thread the marker (or
//! forgets to exist) fails here rather than only in Swift/Kotlin.
//!
//! The stored generation itself has no FFI reader yet — its first production
//! reader is the native persistence layer — so these assert the admission
//! boundary: which markers a restore accepts, and that an out-of-range marker
//! fails closed with `InvalidInput` before any author or store is built.

use std::sync::Arc;

use riot_ffi::{
    open_local_profile_for_starter_catalog_generation,
    open_local_profile_with_database_for_starter_catalog_generation,
    open_profile_from_sealed_identity, open_profile_from_sealed_identity_with_database,
    MobileError, MobileProfile,
};

/// A known wrapping key (32 bytes) for deterministic test identities.
const TEST_WRAPPING_KEY: [u8; 32] = [0x42; 32];

/// Every marker a persisted profile may legitimately carry: `None` is the
/// durable encoding of generation 1, `Some(1)`/`Some(2)` are explicit.
const ACCEPTED_GENERATIONS: [Option<u8>; 3] = [None, Some(1), Some(2)];

/// Markers no persisted profile may carry — corrupt or forward-dated.
const REFUSED_GENERATIONS: [u8; 4] = [0, 3, 4, u8::MAX];

/// The identityless in-memory restore accepts every legitimate marker and
/// refuses everything else. It mints a bootstrap author, so a successful
/// restore must still yield a usable profile.
#[test]
fn identityless_restore_admits_known_generations_and_refuses_the_rest() {
    for generation in ACCEPTED_GENERATIONS {
        let profile: Arc<MobileProfile> =
            open_local_profile_for_starter_catalog_generation(generation)
                .unwrap_or_else(|error| panic!("restore {generation:?}: {error:?}"));
        profile
            .create_public_space("Restored space".into())
            .expect("restored profile is usable");
    }

    for generation in REFUSED_GENERATIONS {
        assert!(
            matches!(
                open_local_profile_for_starter_catalog_generation(Some(generation)),
                Err(MobileError::InvalidInput)
            ),
            "generation {generation} must be refused as InvalidInput"
        );
    }
}

/// The durable counterpart admits the same set. A refused marker must fail
/// before the database is touched, so the path must still be openable
/// afterwards with a legitimate marker.
#[test]
fn durable_identityless_restore_admits_known_generations_and_refuses_the_rest() {
    for generation in ACCEPTED_GENERATIONS {
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("riot.db").to_string_lossy().to_string();
        let profile = open_local_profile_with_database_for_starter_catalog_generation(
            db_path.clone(),
            generation,
        )
        .unwrap_or_else(|error| panic!("durable restore {generation:?}: {error:?}"));
        profile
            .create_public_space("Restored durable space".into())
            .expect("restored durable profile is usable");
    }

    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("riot.db").to_string_lossy().to_string();
    for generation in REFUSED_GENERATIONS {
        assert!(
            matches!(
                open_local_profile_with_database_for_starter_catalog_generation(
                    db_path.clone(),
                    Some(generation)
                ),
                Err(MobileError::InvalidInput)
            ),
            "generation {generation} must be refused as InvalidInput"
        );
    }
    // The refusals were fail-closed, not fail-dirty: the same path still opens.
    open_local_profile_with_database_for_starter_catalog_generation(db_path, Some(2))
        .expect("path is still openable after refused markers");
}

/// The sealed-identity restore seams thread the marker too, and refuse an
/// out-of-range one even when the sealed identity itself is valid — the
/// generation is validated before the wrapping key is consumed.
#[test]
fn sealed_identity_restores_thread_the_generation_marker() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("riot.db").to_string_lossy().to_string();

    let (sealed, original_namespace) = {
        let profile = open_local_profile_with_database_for_starter_catalog_generation(
            db_path.clone(),
            Some(2),
        )
        .expect("open durable profile");
        let sealed = profile
            .seal_identity(TEST_WRAPPING_KEY.to_vec())
            .expect("seal identity");
        (sealed, profile.identity().expect("identity").namespace_id)
    };

    for generation in ACCEPTED_GENERATIONS {
        let restored = open_profile_from_sealed_identity(
            TEST_WRAPPING_KEY.to_vec(),
            sealed.clone(),
            generation,
        )
        .unwrap_or_else(|error| panic!("sealed restore {generation:?}: {error:?}"));
        assert_eq!(
            restored.identity().expect("identity").namespace_id,
            original_namespace,
            "sealed restore must preserve the identity regardless of generation"
        );

        let durable = open_profile_from_sealed_identity_with_database(
            db_path.clone(),
            TEST_WRAPPING_KEY.to_vec(),
            sealed.clone(),
            generation,
        )
        .unwrap_or_else(|error| panic!("durable sealed restore {generation:?}: {error:?}"));
        assert_eq!(
            durable.identity().expect("identity").namespace_id,
            original_namespace,
            "durable sealed restore must preserve the identity regardless of generation"
        );
    }

    for generation in REFUSED_GENERATIONS {
        assert!(
            matches!(
                open_profile_from_sealed_identity(
                    TEST_WRAPPING_KEY.to_vec(),
                    sealed.clone(),
                    Some(generation)
                ),
                Err(MobileError::InvalidInput)
            ),
            "sealed restore must refuse generation {generation}"
        );
        assert!(
            matches!(
                open_profile_from_sealed_identity_with_database(
                    db_path.clone(),
                    TEST_WRAPPING_KEY.to_vec(),
                    sealed.clone(),
                    Some(generation)
                ),
                Err(MobileError::InvalidInput)
            ),
            "durable sealed restore must refuse generation {generation}"
        );
    }
}
