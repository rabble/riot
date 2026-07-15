import WebKit

/// One person as an app sees them: the stable **id** it stores, plus the two
/// halves it draws.
///
/// The id crosses the bridge as LOWERCASE HEX, not bytes: JS has no byte array
/// across `postMessage`, and hex is already the id convention on this bridge
/// (`appIDHex`). It is the FFI `WhoAmI.id` (raw 32 bytes) hex-encoded, nothing
/// more.
///
/// `displayName` arrives from core ALREADY SANITIZED — no separator, no bidi or
/// control characters — which is exactly what makes it safe for the page to
/// flatten the pair into `"{displayName} · {tag}"`. Neither this bridge nor the
/// page may re-sanitize or re-split it; core is the single enforcement point.
public struct BridgeProfile: Equatable {
    public let idHex: String
    public let displayName: String
    public let tag: String

    public init(idHex: String, displayName: String, tag: String) {
        self.idHex = idHex
        self.displayName = displayName
        self.tag = tag
    }
}

/// The data-access surface a WebView bridge needs, decoupled from the concrete
/// FFI session so it can be unit-tested and so hosts wire it up deliberately.
///
/// Values cross this surface as JSON *text*: `put`/`get`/`list` speak the same
/// JSON string the page produced with `JSON.stringify` and consumes with
/// `JSON.parse`.
public protocol AppDataBridging: AnyObject {
    func put(key: String, valueJSON: String) throws
    func get(key: String) throws -> String?
    func list(prefix: String) throws -> [(key: String, valueJSON: String)]
    /// Who the current person is. An app stores `idHex` and NEVER the name: a
    /// name is a claim that can change, and a stored name is a snapshot no later
    /// rename can ever repair.
    func whoami() -> BridgeProfile
    /// Resolves a stored id back to something drawable, at render time.
    ///
    /// An id this device has never seen a profile for is NOT a failure — core
    /// resolves it to the `member` fallback, so a row authored by a peer whose
    /// profile has not synced yet still draws. `nil` means the id itself was
    /// malformed (not 32 bytes of hex), which is a caller bug.
    func profile(idHex: String) -> BridgeProfile?
    /// Tear the underlying Rust execution session down (Unit 0C). After this,
    /// every read/commit through this bridge fails. Called when the app view is
    /// dismissed so no in-flight bridge call outlives the UI.
    func teardownSession()
    /// Whether the underlying execution session is still valid RIGHT NOW — used
    /// to tell a §4.7 invalidation (revoked / namespace-swapped / stale
    /// generation → close to "Return to Tools") apart from an ordinary per-op
    /// rejection (a malformed key → inline error). Both throw the same error.
    func isSessionValid() -> Bool
}

public extension AppDataBridging {
    // Test doubles and non-session bridges have nothing to tear down and are
    // always "valid"; only the real `AppRuntimeDataBridge` overrides these.
    func teardownSession() {}
    func isSessionValid() -> Bool { true }
}

/// Adapter over the Rust-owned `AppExecutionSession` (Unit 0C) for one app.
/// Every read and commit goes through the gated session, which revalidates the
/// app's authority (trust / namespace / generation / not-destroyed) BEFORE it
/// touches data — so a revoked, namespace-swapped, re-approved, or torn-down app
/// cannot read or write even though this bridge object still exists. Values
/// cross the FFI boundary as UTF-8 `Data`.
public final class AppRuntimeDataBridge: AppDataBridging {
    private let execution: AppExecutionSession
    /// The display-name surface. Names are resolved on EVERY call rather than
    /// cached: a cached name would go stale the moment someone renames or a
    /// peer's profile card finally syncs in, which is precisely the staleness
    /// storing the id exists to eliminate. Each call is a mutex + a store read.
    private let profiles: ProfileSession
    /// The host's persistence hook. The bridge performs the gated write itself
    /// (through the execution session) and hands the resulting signed bundle
    /// bytes here so the host can persist them for replay. Absent for isolated
    /// host tests that have no repository — the write still commits, it is just
    /// not persisted across relaunch.
    private let onCommitted: ((_ key: String, _ bundleBytes: Data) throws -> Void)?

    public init(
        execution: AppExecutionSession,
        profiles: ProfileSession,
        onCommitted: ((_ key: String, _ bundleBytes: Data) throws -> Void)? = nil
    ) {
        self.execution = execution
        self.profiles = profiles
        self.onCommitted = onCommitted
    }

    public func put(key: String, valueJSON: String) throws {
        // The gated write returns the canonical committed bytes; hand them to the
        // host to persist. The revalidation happens inside `appDataPutWithReceipt`
        // BEFORE any store mutation, so a revoked app never commits. A persistence
        // failure propagates (same as before), so the page hears "couldn't save".
        let receipt = try execution.appDataPutWithReceipt(key: key, value: Data(valueJSON.utf8))
        try onCommitted?(key, receipt)
    }

    public func get(key: String) throws -> String? {
        guard let data = try execution.appDataGet(key: key) else { return nil }
        return String(decoding: data, as: UTF8.self)
    }

    public func list(prefix: String) throws -> [(key: String, valueJSON: String)] {
        try execution.appDataList(prefix: prefix).map {
            (key: $0.key, valueJSON: String(decoding: $0.value, as: UTF8.self))
        }
    }

    public func whoami() -> BridgeProfile {
        guard let who = try? profiles.whoami() else {
            return BridgeProfile(idHex: "", displayName: "member", tag: "")
        }
        return BridgeProfile(from: who)
    }

    public func profile(idHex: String) -> BridgeProfile? {
        guard let id = Data(riotHex: idHex) else { return nil }
        // An unknown id is not an error here — Rust returns the `member`
        // fallback. Only a wrong-length id throws, and that is the caller bug
        // the nil above and this one both report.
        guard let who = try? profiles.profileFor(id: id) else { return nil }
        return BridgeProfile(from: who)
    }

    public func teardownSession() { execution.invalidate() }

    public func isSessionValid() -> Bool { execution.isValid() }
}

extension BridgeProfile {
    /// The FFI record, with its raw-bytes id hex-encoded for the page.
    init(from who: WhoAmI) {
        self.init(
            idHex: who.id.riotHexString,
            displayName: who.displayName,
            tag: who.tag
        )
    }
}

extension Data {
    var riotHexString: String {
        map { String(format: "%02x", $0) }.joined()
    }

    /// Strict hex decode: ASCII hex digits only, even length. Deliberately does
    /// NOT go through `UInt8(_:radix:)` alone, which would happily read a
    /// leading "+" or "-" as a sign and accept "+1" as a byte. The 32-byte
    /// length rule for a subspace id stays in Rust, its one enforcement point.
    init?(riotHex: String) {
        let ascii = Array(riotHex.utf8)
        guard !ascii.isEmpty, ascii.count % 2 == 0 else { return nil }
        var bytes = [UInt8]()
        bytes.reserveCapacity(ascii.count / 2)
        for pair in stride(from: 0, to: ascii.count, by: 2) {
            guard let high = Self.riotNibble(ascii[pair]),
                  let low = Self.riotNibble(ascii[pair + 1])
            else { return nil }
            bytes.append(high << 4 | low)
        }
        self.init(bytes)
    }

    private static func riotNibble(_ byte: UInt8) -> UInt8? {
        switch byte {
        case UInt8(ascii: "0")...UInt8(ascii: "9"): return byte - UInt8(ascii: "0")
        case UInt8(ascii: "a")...UInt8(ascii: "f"): return byte - UInt8(ascii: "a") + 10
        case UInt8(ascii: "A")...UInt8(ascii: "F"): return byte - UInt8(ascii: "A") + 10
        default: return nil
        }
    }
}

/// Bridges `window.riot` postMessage calls to the app-data store for ONE app.
///
/// HARD CONTRACT: Rust deliberately does NOT trust-gate
/// `app_data_put/get/list` — the WebView host is the enforcement point. A host
/// may only ever construct an `AppBridgeController` (and the
/// `AppRuntimeDataBridge` behind it) for an app that is trusted in the current
/// space; trust is enforced at launch time, and this controller assumes its
/// bridge is already authorized.
@MainActor
public final class AppBridgeController: NSObject, WKScriptMessageHandler {
    /// Total message budget; individual values are further capped in Rust.
    public static let maxMessageBytes = 262_144

    /// Fixed §4.7 copy shown once when a running app's access is invalidated
    /// (trust revoked, namespace swapped, approval changed). The page is closed to
    /// its named destination — it never keeps trying against a dead session.
    public static let revokedMessage = "Your access to this tool was turned off. Return to Tools."

    private let bridge: AppDataBridging
    public weak var webView: WKWebView?
    /// Called after a successful put from this page.
    public var onLocalWrite: (() -> Void)?
    /// Called when a read/commit fails BECAUSE the execution session was
    /// invalidated (§4.7), so the host can close the app to "Return to Tools"
    /// rather than leaving it showing a generic per-op error. Distinct from an
    /// ordinary failure (a malformed key), which leaves the session valid.
    public var onInvalidated: (() -> Void)?

    public init(bridge: AppDataBridging) {
        self.bridge = bridge
    }

    /// Tear the underlying execution session down. The host calls this when the
    /// app view is dismissed; after it, every read/commit fails closed.
    public func teardown() {
        bridge.teardownSession()
    }

    /// Route a caught bridge error. If the session is no longer valid this is a
    /// §4.7 invalidation: reply with the fixed revoked copy and fire
    /// `onInvalidated` so the host closes to Tools. Otherwise it is an ordinary
    /// per-op failure and the page stays open with the caller's message.
    private func replyForError(id: Int, fallback: String) {
        if !bridge.isSessionValid() {
            reply(id: id, ok: false, payloadJSON: jsonString(Self.revokedMessage))
            onInvalidated?()
        } else {
            reply(id: id, ok: false, payloadJSON: jsonString(fallback))
        }
    }

    public func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        _ = handleForTesting(body: message.body)
    }

    /// Returns false when the message is rejected before dispatch —
    /// malformed shape, unknown op, or over the size budget.
    @discardableResult
    public func handleForTesting(body: Any) -> Bool {
        guard let dict = body as? [String: Any],
              let id = dict["id"] as? Int,
              let op = dict["op"] as? String
        else { return false }
        let approximateSize = (dict["key"] as? String ?? "").utf8.count
            + (dict["value"] as? String ?? "").utf8.count
            + (dict["prefix"] as? String ?? "").utf8.count
            + (dict["subject"] as? String ?? "").utf8.count
        guard approximateSize <= Self.maxMessageBytes else {
            reply(id: id, ok: false, payloadJSON: jsonString("Couldn't save that — try again"))
            return false
        }

        switch op {
        case "get":
            guard let key = dict["key"] as? String else { return false }
            do {
                let value = try bridge.get(key: key)
                reply(id: id, ok: true, payloadJSON: value.map(jsonString) ?? "null")
            } catch {
                replyForError(id: id, fallback: "Couldn't load that")
            }
        case "put":
            guard let key = dict["key"] as? String, let value = dict["value"] as? String else { return false }
            do {
                try bridge.put(key: key, valueJSON: value)
                reply(id: id, ok: true, payloadJSON: "null")
                onLocalWrite?()
                notifyDataChanged()
            } catch {
                replyForError(id: id, fallback: "Couldn't save that — try again")
            }
        case "list":
            guard let prefix = dict["prefix"] as? String else { return false }
            do {
                let rows = try bridge.list(prefix: prefix)
                let encoded = rows.map { #"{"key":\#(jsonString($0.key)),"value":\#(jsonString($0.valueJSON))}"# }
                reply(id: id, ok: true, payloadJSON: "[\(encoded.joined(separator: ","))]")
            } catch {
                replyForError(id: id, fallback: "Couldn't load that")
            }
        case "whoami":
            // The id is what the app STORES; displayName/tag are only what it
            // draws right now, and it must re-resolve them every render.
            reply(id: id, ok: true, payloadJSON: profileJSON(bridge.whoami(), includeID: true))
        case "profile":
            // NOT "id": the envelope's own "id" is the promise-correlation id,
            // and `Object.assign({ id, op }, params)` in the shim would let a
            // param of that name overwrite it. The subject id travels as
            // "subject".
            guard let subject = dict["subject"] as? String else { return false }
            guard let who = bridge.profile(idHex: subject) else {
                reply(id: id, ok: false, payloadJSON: jsonString("Couldn't load that"))
                return true
            }
            reply(id: id, ok: true, payloadJSON: profileJSON(who, includeID: false))
        default:
            reply(id: id, ok: false, payloadJSON: jsonString("Unsupported"))
            return false
        }
        return true
    }

    public func notifyDataChanged() {
        webView?.evaluateJavaScript("window.__riotDataChanged && window.__riotDataChanged()")
    }

    private func reply(id: Int, ok: Bool, payloadJSON: String) {
        webView?.evaluateJavaScript("window.__riotResolve(\(id), \(ok), \(payloadJSON))")
    }

    /// `{"id":…,"displayName":…,"tag":…}` — the id only where the app is meant
    /// to store it (`whoami`). `profile(id)` answers a question the caller
    /// already knows the id for, so echoing it back would only invite an app to
    /// re-store a name next to it.
    ///
    /// The two halves stay SEPARATE fields. The page flattens them to
    /// `"{displayName} · {tag}"`; core has already guaranteed `displayName`
    /// cannot contain the separator, so the flattening cannot forge a boundary.
    private func profileJSON(_ who: BridgeProfile, includeID: Bool) -> String {
        let idField = includeID ? #""id":\#(jsonString(who.idHex)),"# : ""
        return "{\(idField)\"displayName\":\(jsonString(who.displayName)),\"tag\":\(jsonString(who.tag))}"
    }

    private func jsonString(_ value: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: [value])
        let array = data.flatMap { String(data: $0, encoding: .utf8) } ?? "[\"\"]"
        return String(array.dropFirst().dropLast())
    }
}
