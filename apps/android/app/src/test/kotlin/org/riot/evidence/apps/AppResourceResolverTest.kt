package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AppResourceResolverTest {
    private val appId = "ab".repeat(32)
    private val resolver = AppResourceResolver(
        appId,
        DecodedAppBundle("index.html", listOf(AppResource("index.html", "text/html", ByteArray(1)))),
    )

    @Test
    fun derivesTwoLabelOriginHostWithinDnsLimits() {
        assertEquals("${"ab".repeat(16)}.${"ab".repeat(16)}.riot-app.invalid", resolver.originHost)
        resolver.originHost.split(".").forEach { label -> assert(label.length <= 63) }
        assertEquals("https://${resolver.originHost}/index.html", resolver.entryUrl)
    }

    @Test
    fun servesOnlyExactPathMatches() {
        assertEquals("text/html", resolver.resolve(resolver.originHost, "/index.html")!!.contentType)
        assertNull(resolver.resolve(resolver.originHost, "/missing.js"))
        assertNull(resolver.resolve(resolver.originHost, "/"))
        assertNull(resolver.resolve("evil.example", "/index.html"))
        assertNull(resolver.resolve(null, "/index.html"))
    }

    @Test
    fun traversalShapedPathsResolveToNothing() {
        assertNull(resolver.resolve(resolver.originHost, "/../index.html"))
        assertNull(resolver.resolve(resolver.originHost, "/..%2Findex.html"))
        assertNull(resolver.resolve(resolver.originHost, "//index.html"))
    }
}
