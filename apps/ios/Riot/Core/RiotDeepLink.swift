import Foundation

/// A parsed `riot://` deep link. The public web newswire ("web = reach") renders
/// signed posts and offers "Open in Riot" links back into the app ("app =
/// truth"); this is the seam that understands them. Two shapes:
///
///  - `riot://open?namespace=<hex>[&entry=<hex>]` — the newswire's home/masthead
///    (no `entry`) and per-post (`entry` set) verify links.
///  - `riot://newswire/join/v1/...` — the existing digest-bound share/join
///    reference. This is carried back VERBATIM so it can be routed to the
///    established `decodeShareReference` join path; it is deliberately NOT
///    re-parsed here (the core codec owns that format).
public enum RiotDeepLink: Equatable, Sendable {
    /// Open a community's Home and, when `entry` is set, verify that one post.
    case openSpace(namespace: String, entry: String?)
    /// A canonical join reference; hand the whole string to the join codec.
    case joinReference(encoded: String)

    /// Parses an incoming URL, or `nil` if it is not a `riot://` link this app
    /// handles. Refusing here (rather than guessing) is what keeps the router
    /// from acting on a link it did not understand.
    public static func parse(_ url: URL) -> RiotDeepLink? {
        guard let scheme = url.scheme, scheme.caseInsensitiveCompare("riot") == .orderedSame else {
            return nil
        }
        // `URLComponents` splits `riot://open?...` into host `open` + query, and
        // `riot://newswire/join/v1/...` into host `newswire` + path.
        guard let host = url.host?.lowercased() else { return nil }
        switch host {
        case "open":
            let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
            let namespace = items.first { $0.name == "namespace" }?.value
            guard let namespace, !namespace.isEmpty else { return nil }
            let entryValue = items.first { $0.name == "entry" }?.value
            let entry = (entryValue?.isEmpty == false) ? entryValue : nil
            return .openSpace(namespace: namespace, entry: entry)
        case "newswire":
            // Only the canonical join reference lives under `newswire/join/`.
            guard url.path.hasPrefix("/join/") else { return nil }
            return .joinReference(encoded: url.absoluteString)
        default:
            return nil
        }
    }

    /// Convenience for callers holding a raw string (e.g. a pasted link).
    public static func parse(string: String) -> RiotDeepLink? {
        URL(string: string).flatMap(parse)
    }
}

/// The honest outcome of following a `riot://open?...` verify link.
///
/// The anti-forgery boundary lives in ONE distinction: `verified` is claimed only
/// for a post the device independently HOLDS as a signed record for a community it
/// follows. Such a record is in the store only because it passed core's
/// Ed25519-verifying import path, so a mirror serving forged HTML cannot make the
/// app show "verified" for a post it never synced — a forged entry id resolves to
/// `postNotHeld`, and an unknown community to `notFollowing`. There is deliberately
/// no green checkmark for anything the app has not cryptographically checked.
public enum RiotOpenOutcome: Equatable, Sendable {
    /// Followed community, and this exact post is held as a signed,
    /// signature-verified record. The honest "Verified in Riot".
    case verified(namespace: String, entry: String, headline: String?)
    /// Followed community, but this post is not in the synced copy yet, so its
    /// signature cannot be checked. Not a forgery verdict either way — sync/refresh.
    case postNotHeld(namespace: String, entry: String)
    /// Home/masthead link (no entry) for a followed community — just open Home.
    case openedHome(namespace: String)
    /// The device does not follow this community, so there is nothing to verify
    /// against — offer to join, then verify after sync.
    case notFollowing(namespace: String, entry: String?)
}

extension RiotOpenOutcome: Identifiable {
    /// A stable id so the outcome can drive a `.sheet(item:)`. Distinct per case
    /// and coordinate so a new link re-presents rather than being coalesced.
    public var id: String {
        switch self {
        case let .verified(ns, entry, _): "verified:\(ns):\(entry)"
        case let .postNotHeld(ns, entry): "notheld:\(ns):\(entry)"
        case let .openedHome(ns): "home:\(ns)"
        case let .notFollowing(ns, entry): "notfollowing:\(ns):\(entry ?? "")"
        }
    }
}

/// Pure decision for a `riot://open?...` link. Kept free of FFI types so the
/// verify-vs-not-member and verified-vs-forged branches are provable without a
/// live store; the caller supplies whether the device follows the namespace and
/// the set of entry ids it actually holds for that community.
public enum RiotDeepLinkResolver {
    /// Resolves an open link from coordinates the caller already has.
    ///
    /// - Parameters:
    ///   - namespace: the community namespace from the link.
    ///   - entry: the per-post entry id, or `nil` for a home/masthead link.
    ///   - followsNamespace: whether the device holds this community.
    ///   - heldEntryIDs: lowercase-hex entry ids present in the community's
    ///     projected, signature-verified wire (empty when unknown/unsynced).
    ///   - headlineForEntry: optional lookup so a verified result can name the post.
    public static func resolveOpen(
        namespace: String,
        entry: String?,
        followsNamespace: Bool,
        heldEntryIDs: Set<String>,
        headlineForEntry: (String) -> String? = { _ in nil }
    ) -> RiotOpenOutcome {
        guard followsNamespace else {
            return .notFollowing(namespace: namespace, entry: entry)
        }
        guard let entry else {
            return .openedHome(namespace: namespace)
        }
        let key = entry.lowercased()
        if heldEntryIDs.contains(key) {
            return .verified(namespace: namespace, entry: entry, headline: headlineForEntry(key))
        }
        return .postNotHeld(namespace: namespace, entry: entry)
    }
}
