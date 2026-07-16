//! Signed JS apps: manifest/bundle format, per-space trust list, and the
//! namespace-scoped data bridge apps use to read/write their own Willow
//! entries. Kept separate from `import/` (evidence-only).

pub mod bridge;
pub mod bundle;
pub mod directory;
pub mod endorse;
pub mod entry;
pub mod index;
pub mod manifest;
pub mod starter;
pub mod trust;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppsError {
    KeyEmpty,
    KeySegmentInvalid,
    TooManyPathComponents,
    PathComponentTooLong,
    PathTooLong,
    PathInvalid,
    Willow(crate::willow::WillowError),
    ManifestFieldInvalid,
    BundleFieldInvalid,
    BundleTooLarge,
    /// The store refused the write (session/budget/admission failure).
    StoreRejected,
    /// A local app-index write would replace an active import review.
    StoreBusy,
    /// The requested local write is older than, or conflicts at the same
    /// timestamp with, the live value at its exact Willow coordinate.
    StaleWrite,
    IndexFieldInvalid,
    EndorsementFieldInvalid,
    IndexEntryMismatch,
    /// The bundle's bytes reference a WebRTC API. `RTCPeerConnection` (and its
    /// `webkit`/`moz` prefixes) and `RTCDataChannel` are the EGRESS vector Risk 9
    /// names: a peer connection does NOT flow through the WebView URL loader, so
    /// the hosted-app egress backstop (`WKContentRuleList`) cannot see or block
    /// it — a hostile app could exfiltrate over STUN/TURN. `getUserMedia` /
    /// `navigator.mediaDevices` are bonus camera/mic CAPTURE blocking, not egress.
    /// We refuse to host any bundle referencing them at the content-scan gate
    /// rather than rely on the best-effort runtime preference alone.
    ///
    /// DENY-CLOSED BY DESIGN: the scan is a substring match over resource bytes,
    /// so it refuses a bundle that merely MENTIONS a token — a comment, a
    /// feature-detect polyfill, a vendored lib that names but never calls the API.
    /// For an activist tool that should not host WebRTC-capable code at all, that
    /// is the correct tradeoff. It is also evadable in the other direction (an
    /// obfuscated or dynamically-assembled identifier passes), so it raises the
    /// bar without being a guarantee; the runtime backstops remain the net. See
    /// `bundle::scan_bundle_egress`.
    BundleUsesWebRtc,
}

impl std::fmt::Display for AppsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AppsError {}

impl From<crate::willow::WillowError> for AppsError {
    fn from(e: crate::willow::WillowError) -> Self {
        AppsError::Willow(e)
    }
}
