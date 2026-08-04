# 0.6.0-alpha.3

Fix identification of existing entries in `PersistentStore::insert_entry` and `MemoryStore::insert_entry`.

# 0.6.0-alpha.2

Avoid redundantly importing duplicate entries from drops.

Correctly retain payloads from verifiable slice streams when working with `PersistentStore`s.

# 0.6.0-alpha.1

#### Breaking

Update `bab_rs` dependency to `0.8.0`, with corresponding updates to all digests.

#### Non-Breaking

Add the `drop_format` module.

# 0.5.0

Update `bab_rs` dependency to `0.6.0`.

# 0.5.0-alpha.1

#### Breaking

Remove the `Arbitrary` impl of `McAuthorisationToken`.

Update `ufotofu` dependency to `0.12.1`.

#### Non-Breaking

Add the `defaults` module.

Add the `storage` module.

Add various encodings.

# 0.4.0

#### Breaking

Remove `Sucesssor` and `Predecessor` trait impls (and impls for the traits
extending them) from `NamespaceId` and `SubspaceId`.

Implement `Entrylike`, `EntrylikeExt` and friends on `?Sized` types.

Rename `into_components` to `into_parts` everywhere for consistency.

#### Non-Breaking

Add suffix and slice creation to `Path`.

Add the `authorisation` module, providing an implementation of Meadowcap for the
willow25 parameters.

# 0.3.0

#### Breaking

Disallow empty groupings, adjust `Grouping` trait and all implementors
accordingly.

Add a new `Namespaced` trait, and make `Entrylike` depend on it.

#### Non-Breaking

Add entry builder payload_digest and payload_size setting from a `BulkProducer`
of the payload bytes.

Add indexing for `Component`s.

## 0.2.1

Add `Component::new_mut`, `Component::new_mut_unchecked`, and
`Component::as_bytes_mut`.

# 0.2.0

#### Breaking

Move `Keylike` to `crate::groupings`.

Remove some `AsRef<willow_data_model::SomeType>` impls.

#### Non-Breaking

Add `groupings` module.

Make wrapper types more consistent in always implementing `From` by value,
reference, and mutable reference (we do not use `AsRef` and `AsMut` in order to
not interfere with more useful `AsRef` and `AsMut` impls).

## 0.1.1

Move the readme to the correct location, so that it is displayed on crates.io.
No changes to the code.

# 0.1.0

Initial release.
