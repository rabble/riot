## 0.8.1

Fixes a bug wherein the filesystem storage backend reports an error if tasked to delete storage which was previously deleted.

## 0.8.0

This truly fixes WILLIAM3 digests. For testing, we have converted the WILLIAM3
implementaiton into a BLAKE3 implementation by changing only those bits that
differ according to the WILLIAM3 specification (IVs, and the `t` argument for
the inner compression function) and verified the resulting BLAKE3 implementation
against the BLAKE3 reference implementation.

## 0.7.0

This fixes an error in the computation of WILLIAM3 digests. All earlier versions
of this crate compute incorrect WILLIAM3 digests. Sorry =S

## 0.6.1

Add `units::optimal_left_skip`.

Add `MultiSliceStore` and related functionality.

## 0.6.0

#### Breaking Changes

Turn `SliceStreamingOptions::layer_filter_factor` into a non-zero u8.

Rename the `"backend"` feature to `"storage`, gate the full `storage` modules
behind it.

#### Non-Breaking Changes

A whole lot of bug fixes around verifiable streaming.

## 0.5.0

#### Breaking Changes

`StoreBackend::initialise_backend` and `SingleSliceStore::create_and_initialise`
panic in case of invalid producer lengths, instead of returning a dedicated
error variant.

Bump `ufotofu` dependency to `0.12.0`.

#### Non-Breaking Changes

Fix some bugs from `0.5.0-alpha.3`.

Add `Display` and `Error` impls for all error types.

Remove `P::Final = ()` bound for `SingleSliceStore::create_and_initialise`.

Add `rename` methods on `StorageBackend` and `SingleSliceStore`.

## 0.5.0-alpha.3

#### Breaking Changes

`SingleSliceStore::get_data` and `SingleSliceStore::determine` do not close the
consumer passed to them any longer, instead they return how many bytes were
written into the consumer when done.

## 0.5.0-alpha.2

#### Breaking Changes

Update `ufotofu` dependency to version `0.11.0`.

## 0.5.0-alpha.1

Add `storage` module(s).

The persistent storage backend is not tested (and probably broken). All
functionality involving nontrivial k-grouping is untested (and almost assuredly
broken).

## 0.4.3

Add `impl AsRef<BabDigest<WIDTH>> for William3Digest` and
`impl AsMut<BabDigest<WIDTH>> for William3Digest`.

Implement the traits of the `order_theory` crate for `BabDigest` and
`William3Digest`.

## 0.4.2

Derive `core::hash::Hash` on `BabDigest` and `William3Digest`.

## 0.4.1

Add the `dev` feature. When it is enabled, `BabDigest` and `William3Digest`
implement the `Arbitrary` trait.

# 0.4.0

Remove the `impl<const WIDTH: usize> From<BabDigest<WIDTH>> for [u8; WIDTH]` and
`impl From<William3Digest> for [u8; WIDTH]` impls, to make it less likely that
users accidentally sidestep constant-time-equality checks and/or zero-on-drop.
Replaces these with non-trait-backed `into_bytes`, `as_bytes`, and
`as_mut_bytes` methods.

# 0.3.0

Introduce proper wrapper types for digests: `BabDigest` and `William3Digest`.
Implement constant-time equality comparisons and zero-on-drop on them. Thank you
@Miaourt for implementing these!

# 0.2.0

- Rename `generic::Hasher` to `generic::BabHasher`, and `william3::Hasher` to
  `william3::William3Hasher`.
- Implement the traits of the [`anyhash`](https://crates.io/crates/anyhash)
  crates, and remove the old non-trait methods of the same names.

# 0.1.0

Initial release.
