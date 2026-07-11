import WebKit

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
    func displayName() -> String
}

/// Adapter over the landed `AppRuntimeSession` FFI for one app id. Values
/// cross the FFI boundary as UTF-8 `Data`.
public final class AppRuntimeDataBridge: AppDataBridging {
    private let session: AppRuntimeSession
    private let appIDHex: String

    public init(session: AppRuntimeSession, appIDHex: String) {
        self.session = session
        self.appIDHex = appIDHex
    }

    public func put(key: String, valueJSON: String) throws {
        try session.appDataPut(appId: appIDHex, key: key, value: Data(valueJSON.utf8))
    }

    public func get(key: String) throws -> String? {
        guard let data = try session.appDataGet(appId: appIDHex, key: key) else { return nil }
        return String(decoding: data, as: UTF8.self)
    }

    public func list(prefix: String) throws -> [(key: String, valueJSON: String)] {
        try session.appDataList(appId: appIDHex, prefix: prefix).map {
            (key: $0.key, valueJSON: String(decoding: $0.value, as: UTF8.self))
        }
    }

    /// v1 placeholder until profiles carry names; a fuller name arrives with a
    /// later FFI addition.
    public func displayName() -> String { "member" }
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

    private let bridge: AppDataBridging
    public weak var webView: WKWebView?
    /// Called after a successful put from this page.
    public var onLocalWrite: (() -> Void)?

    public init(bridge: AppDataBridging) {
        self.bridge = bridge
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
                reply(id: id, ok: false, payloadJSON: jsonString("Couldn't load that"))
            }
        case "put":
            guard let key = dict["key"] as? String, let value = dict["value"] as? String else { return false }
            do {
                try bridge.put(key: key, valueJSON: value)
                reply(id: id, ok: true, payloadJSON: "null")
                onLocalWrite?()
                notifyDataChanged()
            } catch {
                reply(id: id, ok: false, payloadJSON: jsonString("Couldn't save that — try again"))
            }
        case "list":
            guard let prefix = dict["prefix"] as? String else { return false }
            do {
                let rows = try bridge.list(prefix: prefix)
                let encoded = rows.map { #"{"key":\#(jsonString($0.key)),"value":\#(jsonString($0.valueJSON))}"# }
                reply(id: id, ok: true, payloadJSON: "[\(encoded.joined(separator: ","))]")
            } catch {
                reply(id: id, ok: false, payloadJSON: jsonString("Couldn't load that"))
            }
        case "whoami":
            let name = bridge.displayName()
            reply(id: id, ok: true, payloadJSON: #"{"displayName":\#(jsonString(name))}"#)
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

    private func jsonString(_ value: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: [value])
        let array = data.flatMap { String(data: $0, encoding: .utf8) } ?? "[\"\"]"
        return String(array.dropFirst().dropLast())
    }
}
