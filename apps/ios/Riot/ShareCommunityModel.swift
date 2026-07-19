import Foundation

/// The generate side of join, as a pure value the sheet renders. No view
/// dependency and no FFI dependency of its own — the caller injects a closure
/// that resolves a descriptor entry id to the canonical share-reference string
/// (`RiotProfileRepository.newswireShareReference(...).encoded`). That keeps this
/// testable against the real core with an `openLocalProfile()` and no UI, and
/// keeps the anti-substitution digest inside core where it belongs.
public enum ShareCommunityContent: Equatable {
    /// The community's descriptor id isn't known on this device yet (a freshly
    /// joined community before first sync) — nothing to share; show an honest note.
    case unavailable
    /// A canonical `riot://newswire/join/v1/...` link ready to share + encode.
    case shareable(link: String)
}

public struct ShareCommunityModel {
    public init() {}

    /// Resolve the share content for the active community. A nil descriptor id, a
    /// resolver that throws (profile closed / descriptor not held), or a resolver
    /// that returns a non-`riot://` string all collapse to `.unavailable` — never a
    /// crash, never a fabricated or foreign link.
    public func content(
        descriptorEntryID: String?,
        resolveEncoded: (String) throws -> String
    ) -> ShareCommunityContent {
        guard let id = descriptorEntryID,
              let encoded = try? resolveEncoded(id),
              encoded.hasPrefix("riot://newswire/join/v1/") else {
            return .unavailable
        }
        return .shareable(link: encoded)
    }
}
