package org.riot.evidence.apps

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.riot_ffi.openLocalProfile

/**
 * The non-gated definition of done: install the committed checklist from
 * signed bytes, review/trust it, open it in the real hardened WebView, and
 * round-trip items through `window.riot`; adversarial cases prove a hostile
 * page cannot reach the network or escape its data scope.
 */
@RunWith(AndroidJUnit4::class)
class AppRuntimeEndToEndTest {

    private fun evaluate(host: AppWebViewHost, script: String): String? {
        var result: String? = null
        val latch = CountDownLatch(1)
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            host.webView.evaluateJavascript(script) { value ->
                result = value
                latch.countDown()
            }
        }
        latch.await(5, TimeUnit.SECONDS)
        return result
    }

    /** Poll until the script yields a non-null, non-"null" JS value. */
    private fun waitForJs(host: AppWebViewHost, script: String, timeoutMs: Long = 15_000): String {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            val value = evaluate(host, script)
            if (value != null && value != "null") return value
            Thread.sleep(250)
        }
        throw AssertionError("timed out waiting for: $script")
    }

    private fun installTrustedChecklist(): Triple<uniffi.riot_ffi.AppRuntimeSession, InstalledApp, AppWebViewHost> {
        val context = InstrumentationRegistry.getInstrumentation().context // test APK assets
        val target = InstrumentationRegistry.getInstrumentation().targetContext
        val profile = openLocalProfile()
        profile.createPublicSpace("Berlin Mutual Aid")
        val session = profile.appRuntime()
        val apps = RiotAppsController(session)
        val app = apps.install(
            ChecklistFixture.committedManifestBytes(context),
            ChecklistFixture.committedBundleBytes(context),
        )
        assertEquals("Checklist", app.record.name)
        assertFalse(session.isAppTrusted(app.record.appId))
        apps.trust(app)
        assertTrue(session.isAppTrusted(app.record.appId))

        lateinit var host: AppWebViewHost
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val resolver = AppResourceResolver(app.record.appId, app.bundle)
            val bridge = RiotJsBridge(UniffiAppDataPort(session, app.record.appId), "member-e2e")
            host = AppWebViewHost(target, resolver, bridge)
            host.load()
        }
        waitForJs(host, "window.riot ? 'ready' : null")
        return Triple(session, app, host)
    }

    @Test
    fun installTrustOpenAndRoundTripChecklistItems() {
        val (session, app, host) = installTrustedChecklist()

        // Kick-once-then-poll-the-stash: every script evaluates to the
        // stashed outcome or null, so waitForJs can poll it repeatedly.
        val putOutcome = waitForJs(
            host,
            """
            window.__putDone ? window.__putDone :
              ((window.__putKicked = window.__putKicked || window.riot
                .put('items/e2e', {text: 'water', done: false, updated_by: '', updated_at: 1})
                .then(function () { window.__putDone = 'stored'; },
                      function () { window.__putDone = 'failed'; })), null)
            """.trimIndent(),
        )
        assertEquals("stored", putOutcome.trim('"'))

        val stored = session.appDataGet(app.record.appId, "items/e2e")!!.decodeToString()
        assertTrue(stored.contains("water"))

        val listed = waitForJs(
            host,
            """
            window.__rows ? String(window.__rows.length) :
              ((window.__listKicked = window.__listKicked || window.riot.list('items')
                .then(function (rows) { window.__rows = rows; })), null)
            """.trimIndent(),
        )
        assertEquals("1", listed.trim('"'))
    }

    @Test
    fun hostilePageCannotFetchOrEscapeScope() {
        val (_, _, host) = installTrustedChecklist()

        val fetchOutcome = waitForJs(
            host,
            """
            window.__fetch ? window.__fetch :
              (fetch('https://example.com').then(
                function () { window.__fetch = 'FETCHED'; },
                function () { window.__fetch = 'blocked'; }), null)
            """.trimIndent(),
        )
        assertEquals("blocked", fetchOutcome.trim('"'))

        val escapeOutcome = waitForJs(
            host,
            """
            window.__esc ? window.__esc :
              (window.riot.put('../escape', {x: 1}).then(
                function () { window.__esc = 'WROTE'; },
                function () { window.__esc = 'rejected'; }), null)
            """.trimIndent(),
        )
        assertEquals("rejected", escapeOutcome.trim('"'))
    }

    @Test
    fun untrustedAppNeverPassesTheLaunchGate() {
        val context = InstrumentationRegistry.getInstrumentation().context
        val profile = openLocalProfile()
        profile.createPublicSpace("Berlin Mutual Aid")
        val apps = RiotAppsController(profile.appRuntime())
        val app = apps.install(
            ChecklistFixture.committedManifestBytes(context),
            ChecklistFixture.committedBundleBytes(context),
        )
        assertThrows(IllegalStateException::class.java) { apps.requireTrusted(app) }
    }
}
