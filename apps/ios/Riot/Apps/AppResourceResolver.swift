import Foundation

/// Holds one app's decoded bundle and serves its resources by exact path
/// match. iOS reaches resources over a custom `riot-app://<app_id_hex>/<path>`
/// scheme, so — unlike Android's synthetic-origin resolver — there is no
/// origin host to derive here; the app id is kept only so the scheme handler
/// can confirm a request's host belongs to this app.
public struct AppResourceResolver: Sendable {
    /// Lowercased hex app id. Browsers lowercase URL hosts, so comparisons in
    /// the scheme handler are against this normalized form.
    public let appIDHex: String
    public let entryPoint: String
    private let resources: [AppResource]

    public init(appIDHex: String, bundle: DecodedAppBundle) {
        self.appIDHex = appIDHex.lowercased()
        self.entryPoint = bundle.entryPoint
        self.resources = bundle.resources
    }

    /// Exact-match lookup is the traversal defense — no path interpretation
    /// happens at all; "../x" simply matches no resource.
    public func resolve(path: String) -> AppResource? {
        if path.isEmpty { return nil }
        return resources.first { $0.path == path }
    }
}
