//! Riot client internet runtime.
//!
//! This crate owns the ONE process-singleton [`runtime::RiotApplicationRuntime`]
//! that native shells (via `riot-ffi`, WU-012) use for internet operations. A
//! single application process holds one iroh endpoint and one Tokio runtime,
//! constructed once through a [`runtime::RuntimeHost`]; duplicate construction
//! reuses the existing runtime rather than binding a second endpoint.
//!
//! Per-profile [`runtime::ProfileLease`]s scope background work: releasing a
//! lease cancels only that profile's tasks and streams, and orderly application
//! close is refused until every profile lease has been released.
//!
//! The iroh endpoint and Tokio runtime sit behind injected factory traits
//! ([`runtime::EndpointFactory`] / [`runtime::TaskSpawner`]) so the entire
//! lifecycle — singleton reuse, cross-profile cancellation, task drain, and
//! bounded shutdown — is unit-testable with fakes and no live network.

pub mod runtime;
