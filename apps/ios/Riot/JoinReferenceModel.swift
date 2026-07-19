import Foundation

/// The honest, pre-sync preview of a community someone shared a reference to.
///
/// A `riot://newswire/join/v1/...` reference carries only COORDINATES — namespace,
/// descriptor entry, and a content digest — never a name. So the preview leads with
/// a short namespace and deliberately exposes NO title: the real community name
/// arrives on first sync, and the UI must not fabricate one (an attacker who mints a
/// reference cannot choose the name a joiner sees).
public struct JoinPreview: Equatable {
    public let namespaceIdHex: String
    public let descriptorEntryIdHex: String
    public let contentDigestHex: String
    public let encoded: String

    /// Always `nil`: the share reference carries no title (anti-spoof). Present as a
    /// property so call sites read intent, not a magic constant.
    public var title: String? { nil }

    /// A short, human-glanceable namespace prefix — enough to tell two pending joins
    /// apart without leading with a 64-char technical id.
    public var shortNamespace: String { String(namespaceIdHex.prefix(8)) + "…" }
}

/// Why a pasted or scanned string was refused, mapped to actionable copy by the UI.
public enum JoinReferenceError: Error, Equatable {
    /// A scanned payload that is not a `riot://newswire/join/...` link at all.
    case notARiotJoinLink
    /// A canonically-shaped reference the core codec still rejected (truncated,
    /// wrong lengths, corrupt).
    case decodeFailed
    /// The payload exceeds the sane bound — refused before any decode work, so a
    /// hostile QR code (or a pasted blob) cannot make the app chew on megabytes.
    case tooLong
}

/// Pure, camera-free decode + validate + duplicate-detection for the join-by-link /
/// QR flow. Unit-testable without a device: `QRScannerView` merely feeds it a
/// scanned string, and the sheet renders whatever preview or error it returns.
public final class JoinReferenceModel {
    private static let joinScheme = "riot://newswire/join/"
    /// A canonical reference is well under this; the bound only exists to refuse a
    /// hostile oversize payload before the decoder ever sees it.
    static let maxLength = 4096

    public init() {}

    /// Paste path: decode via the shared core codec. Any decode failure — foreign
    /// scheme, truncation, corruption — becomes an actionable `JoinReferenceError`
    /// rather than a raw core error. Length is bounded first.
    public func preview(fromPastedString string: String) throws -> JoinPreview {
        let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count <= Self.maxLength else { throw JoinReferenceError.tooLong }
        let reference: NewswireShareReference
        do {
            reference = try newswireDecodeShareReference(encoded: trimmed)
        } catch {
            throw JoinReferenceError.decodeFailed
        }
        return JoinPreview(
            namespaceIdHex: reference.namespaceId,
            descriptorEntryIdHex: reference.descriptorEntryId,
            contentDigestHex: reference.contentDigest,
            encoded: reference.encoded
        )
    }

    /// Scan path: the input is HOSTILE (any QR code in view), so enforce the
    /// `riot://` join scheme and the length bound BEFORE handing anything to the
    /// decoder. Only then does it share the paste path's decode.
    public func preview(fromScannedString string: String) throws -> JoinPreview {
        guard string.count <= Self.maxLength else { throw JoinReferenceError.tooLong }
        guard string.hasPrefix(Self.joinScheme) else { throw JoinReferenceError.notARiotJoinLink }
        return try preview(fromPastedString: string)
    }

    /// Whether the previewed namespace is already held — in which case the sheet
    /// routes to a switch instead of minting a duplicate row.
    public func isAlreadyJoined(namespaceIdHex: String, within existing: [String]) -> Bool {
        existing.contains { $0.caseInsensitiveCompare(namespaceIdHex) == .orderedSame }
    }
}
