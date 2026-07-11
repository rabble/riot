import WebKit

/// Serves one app's bundle over the custom `riot-app://<app_id_hex>/<path>`
/// scheme. Every response carries the strict, iOS/Android-identical CSP.
/// Resource lookup is an exact string match against the verified bundle
/// (Rust is the verifier) — no path interpretation happens in Swift.
public final class AppSchemeHandler: NSObject, WKURLSchemeHandler {
    public static let scheme = "riot-app"
    public static let csp =
        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:"

    public struct Response {
        public let response: HTTPURLResponse
        public let bytes: Data
    }

    public enum SchemeError: Error { case badURL, notFound }

    private let resolver: AppResourceResolver

    public init(resolver: AppResourceResolver) {
        self.resolver = resolver
    }

    /// Synchronous, testable core. The `WKURLSchemeHandler` conformance is a
    /// thin wrapper over this.
    public func response(for url: URL) throws -> Response {
        guard url.scheme == Self.scheme, let host = url.host else {
            throw SchemeError.badURL
        }
        guard host.lowercased() == resolver.appIDHex else {
            throw SchemeError.notFound
        }
        let path = String(url.path.drop(while: { $0 == "/" }))
        guard !path.isEmpty else { throw SchemeError.badURL }
        guard let resource = resolver.resolve(path: path) else {
            throw SchemeError.notFound
        }
        guard let httpResponse = HTTPURLResponse(
            url: url,
            statusCode: 200,
            httpVersion: "HTTP/1.1",
            headerFields: [
                "Content-Type": resource.contentType,
                "Content-Security-Policy": Self.csp,
                "Content-Length": String(resource.bytes.count),
            ]
        ) else { throw SchemeError.badURL }
        return Response(response: httpResponse, bytes: resource.bytes)
    }

    public func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url else {
            urlSchemeTask.didFailWithError(SchemeError.badURL)
            return
        }
        do {
            let served = try response(for: url)
            urlSchemeTask.didReceive(served.response)
            urlSchemeTask.didReceive(served.bytes)
            urlSchemeTask.didFinish()
        } catch {
            urlSchemeTask.didFailWithError(error)
        }
    }

    public func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {}
}
