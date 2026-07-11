package org.riot.evidence.apps

/**
 * Derives a per-app synthetic https origin and serves the bundle's
 * resources by exact path match. The hex app id is split into two 32-char
 * DNS labels (labels cap at 63 chars), giving every app a distinct origin
 * on `riot-app.invalid`.
 */
class AppResourceResolver(appIdHex: String, private val bundle: DecodedAppBundle) {
    init {
        // A 32-byte app id is exactly 64 hex chars; anything else can't
        // split into the two 32-char DNS labels below.
        require(appIdHex.length == 64) { "app id must be 64 hex chars, was ${appIdHex.length}" }
    }

    // Browsers lowercase the host when forming request URLs, so the origin
    // must be lowercase and comparisons case-insensitive; otherwise an
    // uppercase-hex caller would silently 404 every resource.
    private val normalizedId: String = appIdHex.lowercase()
    val originHost: String =
        "${normalizedId.substring(0, 32)}.${normalizedId.substring(32)}.riot-app.invalid"
    val entryUrl: String = "https://$originHost/${bundle.entryPoint}"

    /**
     * Exact-match lookup is the traversal defense — no path interpretation
     * happens at all; "../x" simply matches no resource.
     */
    fun resolve(host: String?, rawPath: String?): AppResource? {
        if (host == null || !host.equals(originHost, ignoreCase = true)) return null
        if (rawPath == null || !rawPath.startsWith("/")) return null
        val path = rawPath.substring(1)
        if (path.isEmpty()) return null
        return bundle.resources.firstOrNull { it.path == path }
    }
}
