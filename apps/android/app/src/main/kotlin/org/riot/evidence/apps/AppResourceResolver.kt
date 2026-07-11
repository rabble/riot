package org.riot.evidence.apps

/**
 * Derives a per-app synthetic https origin and serves the bundle's
 * resources by exact path match. The hex app id is split into two 32-char
 * DNS labels (labels cap at 63 chars), giving every app a distinct origin
 * on `riot-app.invalid`.
 */
class AppResourceResolver(appIdHex: String, private val bundle: DecodedAppBundle) {
    val originHost: String =
        "${appIdHex.substring(0, 32)}.${appIdHex.substring(32)}.riot-app.invalid"
    val entryUrl: String = "https://$originHost/${bundle.entryPoint}"

    /**
     * Exact-match lookup is the traversal defense — no path interpretation
     * happens at all; "../x" simply matches no resource.
     */
    fun resolve(host: String?, rawPath: String?): AppResource? {
        if (host != originHost || rawPath == null || !rawPath.startsWith("/")) return null
        val path = rawPath.substring(1)
        if (path.isEmpty()) return null
        return bundle.resources.firstOrNull { it.path == path }
    }
}
