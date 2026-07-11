# JS Apps Runtime (Android) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Android twin of the iOS runtime plan — a person on an Android emulator installs the signed checklist into their space, reviews and approves it as the organizer, opens it from the Tools section, and adds/checks items through the real WebView; adversarial host tests prove a hostile page cannot reach the network or escape its data scope. Tasks 1–4 build **only** against the landed `AppRuntimeSession` FFI surface (`install_app`, `trust_app`/`untrust_app`/`is_app_trusted`, `app_data_put/get/list`) — zero Rust/FFI changes. Everything needing unlanded FFI is a gated task at the end.

**Architecture:** Resources are served from an in-memory decoded bundle (strict Kotlin mirror of `apps::bundle::decode_app_bundle`, decoded only after Rust's `install_app` accepted the same bytes) via `shouldInterceptRequest` on a per-app synthetic https origin, with the iOS CSP on every response and `blockNetworkLoads` as a second wall. A `window.riot` shim injected at document start (androidx.webkit) wraps a synchronous `@JavascriptInterface` object whose security boundary is Rust's `AppDataBridge`. Launch is gated host-side on `is_app_trusted` — Rust deliberately does not gate data calls (platform handoff, deferred item 3).

**Tech Stack:** Kotlin (plain Views, single-activity — the existing shell; **no Compose**, matching the app as it exists), android.webkit + androidx.webkit, UniFFI Kotlin bindings over JNA, JUnit4 JVM unit tests + androidx.test instrumented tests (API 36 emulator).

**Spec:** `docs/superpowers/specs/2026-07-11-js-apps-runtime-android-design.md` (and its neighbors: the iOS runtime spec whose decisions it ports, the platform spec `2026-07-11-signed-js-apps-design.md`, and `2026-07-11-app-directory-design.md`).

---

## Before you start

1. Run `git status --short` and read `COLLABORATION.md`. This checkout is shared with other active agents (the iOS runtime session and the app-directory session are executing concurrently). Claim the files of the task you are starting in `COLLABORATION.md` before editing. The iOS-runtime claim owns `fixtures/apps/checklist/` (landed — reuse, never edit: the files are frozen content-hash inputs) and the future FFI additions; the directory claim owns `apps_ffi.rs`/`mobile_state.rs`/`riot-app-cli`. **This plan's Tasks 1–4 touch only `apps/android/` — no collisions.**
2. **JDK:** use Homebrew JDK 17 (`COLLABORATION.md` environment note) — every Gradle command below is written as `JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew …`, run from `apps/android/`.
3. **Native prerequisites:** the Android project does not build Rust. From the repo root run `scripts/conference/build-native-core.sh` once before building (and again after any FFI change lands under you). Gradle consumes `build/generated/riot-ffi/uniffi/` (Kotlin bindings) and `build/native/android/jniLibs/` (see `apps/android/README.md`). If `AppRuntimeSession` methods are missing at compile time, the generated bindings are stale — rebuild, don't debug.
4. **Instrumented tests** need a running API 36 emulator (`connectedDebugAndroidTest`). JVM unit tests (`testDebugUnitTest`) need nothing.
5. **Re-read the landed FFI first** (`crates/riot-ffi/src/apps_ffi.rs`): Tasks 1–4 assume exactly `MobileProfile.app_runtime() -> AppRuntimeSession` with `install_app(manifest_bytes, bundle_bytes) -> InstalledAppRecord { app_id, name, description, version, entry_point, permissions }`, `trust_app/untrust_app(app_id) -> ()`, `is_app_trusted(app_id) -> bool`, `app_data_put(app_id, key, value) -> ()`, `app_data_get -> Option<bytes>`, `app_data_list -> Vec<AppDataItem { key, value }>` (Kotlin: `appRuntime()`, `installApp(...)`, ByteArray for bytes). If the surface has grown since (directory Task 6 / iOS Task 5 landing), prefer the landed listing/resource methods over this plan's in-memory workarounds — the gated tasks below say exactly what to swap.
6. No project-file surgery: Gradle source sets pick up new Kotlin files automatically (contrast with the iOS plan's pbxproj note).

## File Structure

All new runtime code in a new `apps` subpackage; existing files edited only where named.

Main (`apps/android/app/src/main/kotlin/org/riot/evidence/apps/`):
- `CanonicalCbor.kt` — Task 1: minimal canonical CBOR writer/reader primitives (definite lengths, minimal heads)
- `AppBundleCodec.kt` — Task 1: strict decode (+ encode as the canonicality oracle) mirroring `apps::bundle`
- `AppResourceResolver.kt` — Task 1: pure per-app origin derivation + exact-match resource lookup
- `AppWebViewHost.kt` — Task 1 (hardened WebView + interception), extended in Task 2 (bridge + shim wiring)
- `RiotJsShim.kt` — Task 2: the injected `window.riot` document-start script (Kotlin string const)
- `AppDataPort.kt` — Task 2: small port interface + `UniffiAppDataPort` adapter over `AppRuntimeSession`
- `RiotJsBridge.kt` — Task 2: `@JavascriptInterface` object, JVM-testable validation
- `InstalledAppsStore.kt` — Task 3: pure in-memory registry of (record, decoded bundle)
- `RiotAppsController.kt` — Task 3: install/trust/launch-guard over `AppRuntimeSession` + store

Modified:
- `apps/android/app/build.gradle.kts` — Task 1: `androidx.webkit` dependency; Task 4: androidTest assets source dir
- `apps/android/app/src/main/kotlin/org/riot/evidence/RiotController.kt` — Task 3: expose `appRuntime()` + `displayName()`
- `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt` — Task 3: Tools section, review view, install picker, full-screen host

JVM tests (`apps/android/app/src/test/kotlin/org/riot/evidence/apps/`):
- `AppBundleCodecTest.kt` (Task 1), `AppResourceResolverTest.kt` (Task 1), `RiotJsBridgeTest.kt` (Task 2), `InstalledAppsStoreTest.kt` (Task 3)

Instrumented (`apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/`):
- `ChecklistFixture.kt` — Task 4: packs the committed `fixtures/apps/checklist/` sources (exposed as androidTest assets) into canonical manifest+bundle CBOR
- `AppRuntimeEndToEndTest.kt` — Task 4: install → trust → launch → JS round trip + adversarial page + launch gating

---

### Task 1: AppWebViewHost — codec, resolver, hardened WebView

**Files:**
- Create: `apps/AppBundleCodec.kt`, `apps/CanonicalCbor.kt`, `apps/AppResourceResolver.kt`, `apps/AppWebViewHost.kt`
- Create: `test/.../apps/AppBundleCodecTest.kt`, `test/.../apps/AppResourceResolverTest.kt`
- Modify: `apps/android/app/build.gradle.kts` (add `implementation("androidx.webkit:webkit:1.14.0")`)

The codec is a strict Kotlin mirror of `crates/riot-core/src/apps/bundle.rs` — same map/key layout (`map(2){0: entry_point, 1: [map(3){0: path, 1: content_type, 2: bytes}]}`), same bounds (`MAX_BUNDLE_RESOURCES = 32`, `MAX_RESOURCE_PATH_BYTES = 256`, `MAX_RESOURCE_CONTENT_TYPE_BYTES = 64`, `MAX_BUNDLE_TOTAL_BYTES = 1_048_576`), same canonicality proof (decode re-encodes and compares). It is a *serving* mirror, not a security boundary: production only ever decodes bytes `install_app` already accepted; drift shows up as a loud install failure, and Task 4's instrumented test uses `install_app` as the oracle that the encoder matches Rust byte-for-byte.

- [ ] **Step 1: Write the failing JVM tests**

```kotlin
package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

class AppBundleCodecTest {
    private fun sample() = DecodedAppBundle(
        entryPoint = "index.html",
        resources = listOf(
            AppResource("app.js", "text/javascript", "riot.watch();".toByteArray()),
            AppResource("index.html", "text/html", "<!doctype html>".toByteArray()),
        ),
    )

    @Test
    fun decodesTheCanonicalEncodingItProduces() {
        val encoded = AppBundleCodec.encode(sample())
        assertEquals(sample(), AppBundleCodec.decode(encoded))
    }

    @Test
    fun rejectsTrailingBytes() {
        val encoded = AppBundleCodec.encode(sample()) + byteArrayOf(0)
        assertThrows(AppBundleCodecException::class.java) { AppBundleCodec.decode(encoded) }
    }

    @Test
    fun rejectsEveryNonCanonicalSingleByteFlipOrDecodesEqual() {
        // Mirror of apps_codec_hostile.rs's byte-flip property: a flipped
        // byte either fails decode or (if it flipped inside resource bytes)
        // decodes to a *different* bundle — never the same bundle from
        // different bytes.
        val encoded = AppBundleCodec.encode(sample())
        for (index in encoded.indices) {
            val mutated = encoded.copyOf().also { it[index] = (it[index].toInt() xor 0x01).toByte() }
            val decoded = runCatching { AppBundleCodec.decode(mutated) }.getOrNull() ?: continue
            assertEquals(mutated.toList(), AppBundleCodec.encode(decoded).toList())
        }
    }

    @Test
    fun rejectsMissingEntryPointResource() {
        val bundle = DecodedAppBundle("missing.html", sample().resources)
        assertThrows(AppBundleCodecException::class.java) { AppBundleCodec.encode(bundle) }
    }

    @Test
    fun rejectsOversizedResourceCountHeaderWithoutAllocating() {
        // map(2), key 0, "a" entry point, key 1, array claiming 2^32 items.
        val forged = byteArrayOf(
            0xA2.toByte(), 0x00, 0x61, 0x61, 0x01,
            0x9A.toByte(), 0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte(),
        )
        assertThrows(AppBundleCodecException::class.java) { AppBundleCodec.decode(forged) }
    }
}

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
```

Run: `JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest` (from `apps/android/`) — expected: compile failure on the new types.

- [ ] **Step 2: Implement `CanonicalCbor.kt` + `AppBundleCodec.kt`**

Writer: one `head(major: Int, value: Long)` emitting the minimal-length argument (direct < 24, then 1/2/4/8-byte forms), plus `text`, `bytes`, `map`, `array` helpers. Reader: strict — definite lengths only, exact expected keys in order (0 then 1; 0/1/2 per resource), type-checked text/bytes, bounds enforced *before* allocation sized from input (the forged-count test), `position == input.size` at the end, then the canonicality proof:

```kotlin
data class AppResource(val path: String, val contentType: String, val bytes: ByteArray) { /* equals/hashCode over content */ }
data class DecodedAppBundle(val entryPoint: String, val resources: List<AppResource>)
class AppBundleCodecException(message: String) : Exception(message)

object AppBundleCodec {
    const val MAX_BUNDLE_RESOURCES = 32
    const val MAX_RESOURCE_PATH_BYTES = 256
    const val MAX_RESOURCE_CONTENT_TYPE_BYTES = 64
    const val MAX_BUNDLE_TOTAL_BYTES = 1_048_576

    fun encode(bundle: DecodedAppBundle): ByteArray { /* validate, then map(2){0,1} as above */ }

    fun decode(input: ByteArray): DecodedAppBundle {
        /* strict read + validate */
        val bundle = /* ... */
        if (!encode(bundle).contentEquals(input)) {
            throw AppBundleCodecException("non-canonical bundle encoding")
        }
        return bundle
    }
}
```

Validation mirrors `bundle.rs::validate` exactly: 1–32 resources, non-empty path/content-type within byte caps, entry point present among resource paths, total resource bytes ≤ 1 MiB, encoded size ≤ 1 MiB.

- [ ] **Step 3: Implement `AppResourceResolver.kt`**

```kotlin
class AppResourceResolver(appIdHex: String, private val bundle: DecodedAppBundle) {
    val originHost: String =
        "${appIdHex.substring(0, 32)}.${appIdHex.substring(32)}.riot-app.invalid"
    val entryUrl: String = "https://$originHost/${bundle.entryPoint}"

    /// Exact-match lookup is the traversal defense — no path
    /// interpretation happens at all; "../x" simply matches no resource.
    fun resolve(host: String?, rawPath: String?): AppResource? {
        if (host != originHost || rawPath == null || !rawPath.startsWith("/")) return null
        val path = rawPath.substring(1)
        if (path.isEmpty()) return null
        return bundle.resources.firstOrNull { it.path == path }
    }
}
```

- [ ] **Step 4: Implement `AppWebViewHost.kt`**

Hardening + interception only in this task (bridge/shim arrive in Task 2). Keep the `WebViewClient` a named inner class so Task 4 can drive it through a real WebView.

```kotlin
package org.riot.evidence.apps

import android.annotation.SuppressLint
import android.content.Context
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import java.io.ByteArrayInputStream

class AppWebViewHost(context: Context, private val resolver: AppResourceResolver) {
    companion object {
        const val CSP =
            "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:"
    }

    val webView: WebView = WebView(context)

    init {
        @SuppressLint("SetJavaScriptEnabled") // The bridge is the app's only I/O; see spec.
        webView.settings.apply {
            javaScriptEnabled = true
            blockNetworkLoads = true
            allowFileAccess = false
            allowContentAccess = false
            domStorageEnabled = false
            javaScriptCanOpenWindowsAutomatically = false
            setSupportMultipleWindows(false)
            cacheMode = WebSettings.LOAD_NO_CACHE
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
        }
        webView.webViewClient = object : WebViewClient() {
            override fun shouldInterceptRequest(
                view: WebView,
                request: WebResourceRequest,
            ): WebResourceResponse {
                val resource = resolver.resolve(request.url.host, request.url.path)
                    ?: return WebResourceResponse(
                        "text/plain", null, 404, "Not Found",
                        mapOf("Content-Security-Policy" to CSP),
                        ByteArrayInputStream(ByteArray(0)),
                    )
                return WebResourceResponse(
                    resource.contentType, null, 200, "OK",
                    mapOf("Content-Security-Policy" to CSP, "Cache-Control" to "no-store"),
                    ByteArrayInputStream(resource.bytes),
                )
            }

            override fun shouldOverrideUrlLoading(
                view: WebView,
                request: WebResourceRequest,
            ): Boolean = request.url.scheme != "https" || request.url.host != resolver.originHost
        }
    }

    fun load() = webView.loadUrl(resolver.entryUrl)
    fun destroy() = webView.destroy()
}
```

- [ ] **Step 5: Run the tests + build**

Run (from `apps/android/`):
```sh
JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest
JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew assembleDebug
```
Expected: all unit tests green (`AppBundleCodecTest` 5, `AppResourceResolverTest` 3, plus the existing suite), APK assembles. (Run `scripts/conference/build-native-core.sh` from the repo root first if `build/generated/riot-ffi/` is missing.)

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/org/riot/evidence/apps/ apps/android/app/src/test/kotlin/org/riot/evidence/apps/ apps/android/app/build.gradle.kts
git commit -m "feat(android): app bundle codec, resource resolver, hardened app WebView host"
```

---

### Task 2: `window.riot` bridge — shim, port, @JavascriptInterface

**Files:**
- Create: `apps/RiotJsShim.kt`, `apps/AppDataPort.kt`, `apps/RiotJsBridge.kt`
- Modify: `apps/AppWebViewHost.kt` (wire bridge + document-start shim)
- Create: `test/.../apps/RiotJsBridgeTest.kt`

`@JavascriptInterface` methods are synchronous and run on WebView's dedicated bridge thread; `AppRuntimeSession` is safe there (it serializes internally over the shared profile mutex), and nothing in Tasks 1–4 touches `PersistedProfile` from the bridge (persistence is gated — re-check thread ownership when gated Task 5 lands). The bridge is pure Kotlin over a port interface so JVM tests never need Android or the FFI; envelopes are hand-rolled JSON (no `org.json` — it is a stub on the JVM test classpath).

- [ ] **Step 1: Write the failing JVM tests**

```kotlin
package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

private class FakeAppDataPort : AppDataPort {
    val stored = mutableMapOf<String, ByteArray>()
    var failNext: Boolean = false
    override fun put(key: String, value: ByteArray) {
        if (failNext) throw IllegalStateException("boom")
        stored[key] = value
    }
    override fun get(key: String): ByteArray? = stored[key]
    override fun list(prefix: String): List<Pair<String, ByteArray>> =
        stored.filterKeys { it.startsWith(prefix) }.map { it.key to it.value }.sortedBy { it.first }
}

class RiotJsBridgeTest {
    private val port = FakeAppDataPort()
    private val bridge = RiotJsBridge(port, displayName = "member-ab12cd34")

    @Test
    fun putStoresJsonTextAndReturnsOkEnvelope() {
        assertEquals("""{"ok":true,"value":null}""", bridge.riotPut("items/x", """{"text":"water"}"""))
        assertEquals("""{"text":"water"}""", port.stored["items/x"]!!.decodeToString())
    }

    @Test
    fun putRejectsOversizedValuesBeforeTouchingThePort() {
        val result = bridge.riotPut("items/x", "a".repeat(RiotJsBridge.MAX_MESSAGE_BYTES + 1))
        assertEquals("""{"ok":false,"error":"Couldn't save that — try again"}""", result)
        assertTrue(port.stored.isEmpty())
    }

    @Test
    fun putRejectsMissingOrEmptyKeysBeforeTouchingThePort() {
        assertTrue(bridge.riotPut(null, "{}").contains("\"ok\":false"))
        assertTrue(bridge.riotPut("", "{}").contains("\"ok\":false"))
        assertTrue(bridge.riotPut("items/x", null).contains("\"ok\":false"))
        assertTrue(port.stored.isEmpty())
    }

    @Test
    fun portFailuresBecomePlainLanguageErrorEnvelopes() {
        port.failNext = true
        assertEquals(
            """{"ok":false,"error":"Couldn't save that — try again"}""",
            bridge.riotPut("items/x", "{}"),
        )
    }

    @Test
    fun getReturnsNullEnvelopeWhenAbsentAndEscapedTextWhenPresent() {
        assertEquals("""{"ok":true,"value":null}""", bridge.riotGet("items/none"))
        port.stored["items/q"] = """{"text":"say \"hi\""}""".toByteArray()
        val envelope = bridge.riotGet("items/q")
        assertTrue(envelope.startsWith("""{"ok":true,"value":""""))
        assertTrue(envelope.contains("\\\"hi\\\""))
    }

    @Test
    fun listReturnsKeyValueRowsAsEscapedJson() {
        port.stored["items/a"] = """{"done":false}""".toByteArray()
        port.stored["items/b"] = """{"done":true}""".toByteArray()
        assertEquals(
            """{"ok":true,"value":[{"key":"items/a","value":"{\"done\":false}"},{"key":"items/b","value":"{\"done\":true}"}]}""",
            bridge.riotList("items"),
        )
    }

    @Test
    fun whoamiReturnsDisplayNameOnly() {
        assertEquals("""{"ok":true,"value":{"displayName":"member-ab12cd34"}}""", bridge.riotWhoami())
    }
}
```

Run: `JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest` — expected: compile failure on the new types.

- [ ] **Step 2: Implement `AppDataPort.kt`**

```kotlin
package org.riot.evidence.apps

import uniffi.riot_ffi.AppRuntimeSession

interface AppDataPort {
    fun put(key: String, value: ByteArray)
    fun get(key: String): ByteArray?
    fun list(prefix: String): List<Pair<String, ByteArray>>
}

/** Thin adapter; prefix scoping, value caps, and key validation live in Rust. */
class UniffiAppDataPort(
    private val session: AppRuntimeSession,
    private val appIdHex: String,
) : AppDataPort {
    override fun put(key: String, value: ByteArray) = session.appDataPut(appIdHex, key, value)
    override fun get(key: String): ByteArray? = session.appDataGet(appIdHex, key)
    override fun list(prefix: String): List<Pair<String, ByteArray>> =
        session.appDataList(appIdHex, prefix).map { it.key to it.value }
}
```

- [ ] **Step 3: Implement `RiotJsBridge.kt`**

```kotlin
package org.riot.evidence.apps

import android.webkit.JavascriptInterface

class RiotJsBridge(private val port: AppDataPort, private val displayName: String) {
    companion object {
        /** Total message budget; individual values are further capped in Rust. */
        const val MAX_MESSAGE_BYTES = 262_144
        private const val SAVE_ERROR = "Couldn't save that — try again"
        private const val LOAD_ERROR = "Couldn't load that"
    }

    @JavascriptInterface
    fun riotPut(key: String?, valueJson: String?): String {
        if (key.isNullOrEmpty() || valueJson == null) return error(SAVE_ERROR)
        if (key.toByteArray().size + valueJson.toByteArray().size > MAX_MESSAGE_BYTES) {
            return error(SAVE_ERROR)
        }
        return runCatching { port.put(key, valueJson.toByteArray()) }
            .fold({ """{"ok":true,"value":null}""" }, { error(SAVE_ERROR) })
    }

    @JavascriptInterface
    fun riotGet(key: String?): String {
        if (key.isNullOrEmpty() || key.toByteArray().size > MAX_MESSAGE_BYTES) return error(LOAD_ERROR)
        return runCatching { port.get(key) }.fold(
            { value ->
                if (value == null) """{"ok":true,"value":null}"""
                else """{"ok":true,"value":${jsonQuote(value.decodeToString())}}"""
            },
            { error(LOAD_ERROR) },
        )
    }

    @JavascriptInterface
    fun riotList(prefix: String?): String {
        if (prefix == null || prefix.toByteArray().size > MAX_MESSAGE_BYTES) return error(LOAD_ERROR)
        return runCatching { port.list(prefix) }.fold(
            { rows ->
                val encoded = rows.joinToString(",") { (key, value) ->
                    """{"key":${jsonQuote(key)},"value":${jsonQuote(value.decodeToString())}}"""
                }
                """{"ok":true,"value":[$encoded]}"""
            },
            { error(LOAD_ERROR) },
        )
    }

    @JavascriptInterface
    fun riotWhoami(): String = """{"ok":true,"value":{"displayName":${jsonQuote(displayName)}}}"""

    private fun error(message: String) = """{"ok":false,"error":${jsonQuote(message)}}"""

    private fun jsonQuote(value: String): String = buildString {
        append('"')
        value.forEach { c ->
            when {
                c == '"' -> append("\\\"")
                c == '\\' -> append("\\\\")
                c == '\n' -> append("\\n")
                c == '\r' -> append("\\r")
                c == '\t' -> append("\\t")
                c < ' ' -> append("\\u%04x".format(c.code))
                else -> append(c)
            }
        }
        append('"')
    }
}
```

- [ ] **Step 4: Implement `RiotJsShim.kt` and wire the host**

The shim keeps the `window.riot` contract byte-identical to iOS's `RiotJS.swift` from the app's point of view (`get`/`put`/`list`/`watch`/`whoami`, Promises, segment-prefix normalization), so `fixtures/apps/checklist/app.js` runs unmodified:

```kotlin
package org.riot.evidence.apps

object RiotJsShim {
    // Not `const`: the ${'$'} escape below makes this a non-constant expression.
    val SOURCE = """
    (function () {
      if (window.riot) { return; }
      var watchers = [];
      function call(fn) {
        return new Promise(function (resolve, reject) {
          var envelope;
          try { envelope = JSON.parse(fn()); } catch (e) { reject(new Error("bridge unavailable")); return; }
          if (envelope.ok) { resolve(envelope.value); } else { reject(new Error(String(envelope.error))); }
        });
      }
      function fireWatchers() {
        watchers.forEach(function (w) {
          window.riot.list(w.prefix).then(w.cb).catch(function () {});
        });
      }
      window.__riotDataChanged = fireWatchers;
      window.riot = {
        get: function (key) {
          return call(function () { return RiotNative.riotGet(String(key)); })
            .then(function (v) { return v == null ? null : JSON.parse(v); });
        },
        put: function (key, value) {
          return call(function () { return RiotNative.riotPut(String(key), JSON.stringify(value)); })
            .then(function () { fireWatchers(); });
        },
        list: function (prefix) {
          var clean = String(prefix).replace(/\/+${'$'}/, "");
          return call(function () { return RiotNative.riotList(clean); })
            .then(function (rows) {
              return rows.map(function (r) { return { key: r.key, value: JSON.parse(r.value) }; });
            });
        },
        watch: function (prefix, cb) {
          watchers.push({ prefix: prefix, cb: cb });
          window.riot.list(prefix).then(cb).catch(function () {});
        },
        whoami: function () { return call(function () { return RiotNative.riotWhoami(); }); },
      };
    })();
    """
}
```

In `AppWebViewHost`, add a constructor parameter `bridge: RiotJsBridge` and, in `init` before any load:

```kotlin
if (!WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
    throw AppHostUnavailableException("This tool can't run on this phone yet")
}
webView.addJavascriptInterface(bridge, "RiotNative")
WebViewCompat.addDocumentStartJavaScript(
    webView, RiotJsShim.SOURCE, setOf("https://${resolver.originHost}"),
)
```

(`androidx.webkit.WebViewCompat` / `WebViewFeature` — fail closed, per spec.) Add `fun notifyDataChanged() = webView.evaluateJavascript("window.__riotDataChanged && window.__riotDataChanged()", null)` for the resume/sync triggers.

- [ ] **Step 5: Run tests + build, commit**

```sh
JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest assembleDebug
```
Expected: all green (7 new `RiotJsBridgeTest` tests), APK assembles.

```bash
git add apps/android/app/src/main/kotlin/org/riot/evidence/apps/ apps/android/app/src/test/kotlin/org/riot/evidence/apps/RiotJsBridgeTest.kt
git commit -m "feat(android): window.riot bridge with document-start shim"
```

---

### Task 3: Tools section — install from bytes, organizer review, trust gating

**Files:**
- Create: `apps/InstalledAppsStore.kt`, `apps/RiotAppsController.kt`
- Create: `test/.../apps/InstalledAppsStoreTest.kt`
- Modify: `RiotController.kt`, `MainActivity.kt`

UI is plain programmatic Views in the existing single-activity shell (the app has no Compose dependency; do not add one). The Tools section lives inside the Spaces surface, matching iOS's placement in the space view; the running app swaps the content container full-screen with a Close button — no second Activity, no second `MobileProfile`.

- [ ] **Step 1: Write the failing JVM test**

```kotlin
package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.riot_ffi.InstalledAppRecord

class InstalledAppsStoreTest {
    private fun record(id: String) = InstalledAppRecord(
        appId = id, name = "Checklist", description = "d", version = "1.0.0",
        entryPoint = "index.html", permissions = listOf("Keep its own notes in this space"),
    )
    private fun bundle() = DecodedAppBundle(
        "index.html", listOf(AppResource("index.html", "text/html", ByteArray(1))),
    )

    @Test
    fun registeringTwiceKeepsOneRowPerAppId() {
        val store = InstalledAppsStore()
        store.register(record("aa".repeat(32)), bundle())
        store.register(record("aa".repeat(32)), bundle())
        assertEquals(1, store.all().size)
    }

    @Test
    fun findsByAppId() {
        val store = InstalledAppsStore()
        store.register(record("aa".repeat(32)), bundle())
        assertEquals("Checklist", store.find("aa".repeat(32))!!.record.name)
        assertEquals(null, store.find("bb".repeat(32)))
    }
}
```

(UniFFI records are plain Kotlin data classes — constructible in JVM tests; match the generated field names in `build/generated/riot-ffi/uniffi/` if they differ from this camelCase guess, and adjust.)

Run: `JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest` — expected: compile failure.

- [ ] **Step 2: Implement store + controller**

```kotlin
data class InstalledApp(val record: InstalledAppRecord, val bundle: DecodedAppBundle)

class InstalledAppsStore {
    private val apps = LinkedHashMap<String, InstalledApp>()
    fun register(record: InstalledAppRecord, bundle: DecodedAppBundle): InstalledApp =
        InstalledApp(record, bundle).also { apps[record.appId] = it }
    fun all(): List<InstalledApp> = apps.values.toList()
    fun find(appIdHex: String): InstalledApp? = apps[appIdHex]
}
```

```kotlin
class RiotAppsController(private val session: AppRuntimeSession) {
    private val store = InstalledAppsStore()

    /** Rust is the integrity oracle: installApp must accept the bytes
     *  before the Kotlin serving-decode ever runs. In-memory retention is
     *  the documented stopgap until app_resource lands (spec, gated). */
    fun install(manifestBytes: ByteArray, bundleBytes: ByteArray): InstalledApp {
        val record = session.installApp(manifestBytes, bundleBytes)
        val bundle = AppBundleCodec.decode(bundleBytes)
        check(bundle.entryPoint == record.entryPoint) { "That file isn't a Riot tool" }
        return store.register(record, bundle)
    }

    fun apps(): List<InstalledApp> = store.all()
    fun isTrusted(app: InstalledApp): Boolean = session.isAppTrusted(app.record.appId)
    fun trust(app: InstalledApp) = session.trustApp(app.record.appId)

    /** The launch gate. Rust does not gate data calls — this is the
     *  enforcement point (platform handoff, deferred item 3). */
    fun requireTrusted(app: InstalledApp): InstalledApp {
        check(isTrusted(app)) { "Ask an organizer to turn this on" }
        return app
    }
}
```

In `RiotController.kt`, add (following the existing thin-delegation style — `identity()` is the precedent):

```kotlin
fun openAppRuntime(): AppRuntimeSession = profile.appRuntime()
fun displayName(): String = "member-" + identity().signingKeyId.take(8)
```

(`identity().signingKeyId` is the existing 64-hex string the binding tests already assert on; the 8-char prefix mirrors the iOS plan's placeholder display name and never exposes the full id.)

- [ ] **Step 3: Wire `MainActivity`**

State: `private lateinit var apps: RiotAppsController` (init in `onCreate` after `controller`), `private var runningApp: Pair<InstalledApp, AppWebViewHost>? = null`, `private var pendingAppManifest: ByteArray? = null`, request codes `PICK_APP_MANIFEST = 11`, `PICK_APP_BUNDLE = 12`.

In `showSpaces()`, after the existing create-space controls, when `controller.currentSpace != null`:

```kotlin
content.addView(heading("Tools"))
if (apps.apps().isEmpty()) {
    content.addView(body("No tools yet. Add a signed tool to this space."))
}
apps.apps().forEach { app ->
    if (apps.isTrusted(app)) {
        content.addView(action("Open ${app.record.name}") { openApp(app) })
    } else {
        content.addView(action("${app.record.name} — New — Review") { showAppReview(app) })
    }
}
content.addView(action("Add a tool (choose manifest, then bundle)") {
    startActivityForResult(openDocumentIntent(), PICK_APP_MANIFEST)
})
```

`onActivityResult` additions (same `BoundedInput.read` + `runAction` pattern as `IMPORT_DOCUMENT`; caps: 4_096 for the manifest, `AppBundleCodec.MAX_BUNDLE_TOTAL_BYTES` for the bundle): `PICK_APP_MANIFEST` stores the bytes and immediately launches the bundle picker; `PICK_APP_BUNDLE` calls `apps.install(pendingAppManifest!!, bytes)` inside `runAction("Tool added — review it under Tools")`, clears `pendingAppManifest`, re-shows SPACES. Any `install_app` rejection surfaces as "That file isn't a Riot tool" via the status line (map the FFI exception message in `runAction`'s catch or wrap install).

Review view (plain language only — the trust-decision moment, mirroring the iOS review sheet):

```kotlin
private fun showAppReview(app: InstalledApp) {
    content.removeAllViews()
    content.addView(heading(app.record.name))
    content.addView(body(app.record.description))
    content.addView(heading("This app can"))
    app.record.permissions.forEach { content.addView(body(it)) }
    content.addView(action("Let everyone in this space use this") {
        runAction("${app.record.name} is on for this space") {
            apps.trust(app)
            show(ConferenceSurface.SPACES)
        }
    })
    content.addView(action("Not now") { show(ConferenceSurface.SPACES) })
}
```

Launch + close (full-screen swap of the content container; `show()` tears down any running host first):

```kotlin
private fun openApp(app: InstalledApp) {
    runAction("Opened ${app.record.name}") {
        val gated = apps.requireTrusted(app)
        val resolver = AppResourceResolver(gated.record.appId, gated.bundle)
        val bridge = RiotJsBridge(
            UniffiAppDataPort(controller.openAppRuntime(), gated.record.appId),
            controller.displayName(),
        )
        val host = AppWebViewHost(this, resolver, bridge)
        runningApp = gated to host
        content.removeAllViews()
        content.addView(action("Close ${gated.record.name}") { closeApp() })
        content.addView(host.webView, weighted())
        host.load()
    }
}

private fun closeApp() {
    runningApp?.second?.destroy()
    runningApp = null
    show(ConferenceSurface.SPACES)
}
```

Also: call `closeApp()` at the top of `show(...)` when `runningApp != null`; in `onResume()`, `runningApp?.second?.notifyDataChanged()` (the spec's foreground `watch` trigger); in `onDestroy()`, destroy any running host before `controller.close()`.

- [ ] **Step 4: Run tests + build, commit**

```sh
JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest assembleDebug
```
Expected: green (2 new `InstalledAppsStoreTest` tests), APK assembles. (Full behavior is proven on-device in Task 4.)

```bash
git add apps/android/app/src/main/kotlin/org/riot/evidence/ apps/android/app/src/test/kotlin/org/riot/evidence/apps/InstalledAppsStoreTest.kt
git commit -m "feat(android): Tools section with install, organizer review, trust gating"
```

---

### Task 4: End-to-end instrumented test through the real WebView

**Files:**
- Create: `androidTest/.../apps/ChecklistFixture.kt`, `androidTest/.../apps/AppRuntimeEndToEndTest.kt`
- Modify: `apps/android/app/build.gradle.kts` (androidTest assets source dir)

The checklist fixture is the iOS session's committed Task 1 output — `fixtures/apps/checklist/{index.html,app.js,style.css,riot-app.json}` (verified present in this checkout). It is reused byte-for-byte as androidTest assets; do not copy or edit the files (they are frozen content-hash inputs). Because no packed `.cbor` artifacts exist yet (gated on the directory plan's CLI), the test packs the sources itself with the Task 1 canonical encoder plus a manifest encoder mirroring `apps::manifest::encode_manifest` — `install_app`'s strict decoder is the oracle proving the Kotlin encoding is byte-exact.

- [ ] **Step 1: Expose the fixture sources as androidTest assets**

In `apps/android/app/build.gradle.kts`, inside the existing `sourceSets` block (mirroring the `getByName("main")` directory-add pattern already there):

```kotlin
getByName("androidTest") {
    assets.directories.add(rootProject.file("../../fixtures/apps").path)
}
```

(`rootProject` is `apps/android/`, so `../../fixtures/apps` is the repo-root fixtures directory — same convention as the existing `build/generated` and `jniLibs` lines.)

- [ ] **Step 2: Write `ChecklistFixture.kt`**

Manifest layout mirrors `apps::manifest::encode_manifest` exactly: `map(9)` with integer keys 0–8 — name, description, version, author namespace_id (32 raw bytes), author subspace_id (32 bytes), namespace_kind (`0` = Communal), signing_key_id (32 bytes), permissions (text array), entry_point. `install_app` performs no authorship verification (integrity is content-addressing), so the author is a fixed committed *public* placeholder identity — same precedent as the conference fixture and the iOS starter-catalog correction (`afae443` lineage); pin the bytes so the derived `app_id` is stable within a test run.

```kotlin
package org.riot.evidence.apps

import android.content.Context
import org.json.JSONObject

object ChecklistFixture {
    // Fixed public placeholder author (never key material; install_app
    // verifies content, not authorship).
    private val NAMESPACE_ID = ByteArray(32) { 0x11 }
    private val SUBSPACE_ID = ByteArray(32) { 0x22 }
    private val SIGNING_KEY_ID = ByteArray(32) { 0x33 }

    private val CONTENT_TYPES = mapOf(
        "index.html" to "text/html",
        "app.js" to "text/javascript",
        "style.css" to "text/css",
    )

    fun manifestBytes(context: Context): ByteArray {
        val source = JSONObject(readAsset(context, "checklist/riot-app.json").decodeToString())
        // CanonicalCbor writer: map(9), keys 0..8 in order, exactly as
        // crates/riot-core/src/apps/manifest.rs::encode_manifest.
        return CanonicalCbor.build { out ->
            out.map(9)
            out.uint(0); out.text(source.getString("name"))
            out.uint(1); out.text(source.getString("description"))
            out.uint(2); out.text(source.getString("version"))
            out.uint(3); out.bytes(NAMESPACE_ID)
            out.uint(4); out.bytes(SUBSPACE_ID)
            out.uint(5); out.uint(0) // NamespaceKind::Communal
            out.uint(6); out.bytes(SIGNING_KEY_ID)
            val permissions = source.getJSONArray("permissions")
            out.uint(7); out.array(permissions.length().toLong())
            (0 until permissions.length()).forEach { out.text(permissions.getString(it)) }
            out.uint(8); out.text(source.getString("entry_point"))
        }
    }

    fun bundleBytes(context: Context): ByteArray {
        val resources = CONTENT_TYPES.entries
            .map { (name, contentType) ->
                AppResource(name, contentType, readAsset(context, "checklist/$name"))
            }
            .sortedBy { it.path }
        return AppBundleCodec.encode(DecodedAppBundle("index.html", resources))
    }

    private fun readAsset(context: Context, path: String): ByteArray =
        context.assets.open(path).use { it.readBytes() }
}
```

(Adapt `CanonicalCbor.build` to Task 1's actual writer API. `org.json` is real on-device — androidTest only.)

- [ ] **Step 3: Write `AppRuntimeEndToEndTest.kt`**

Pattern: `openLocalProfile()` + real FFI, like `BindingSemanticsTest`; the WebView runs on the main thread via `InstrumentationRegistry` without needing an Activity window; JS results are polled because page load and bridge round-trips are async.

```kotlin
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
            ChecklistFixture.manifestBytes(context),
            ChecklistFixture.bundleBytes(context),
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
        val putOutcome = waitForJs(host, """
            window.__putDone ? window.__putDone :
              ((window.__putKicked = window.__putKicked || window.riot
                .put('items/e2e', {text: 'water', done: false, updated_by: '', updated_at: 1})
                .then(function () { window.__putDone = 'stored'; },
                      function () { window.__putDone = 'failed'; })), null)
        """.trimIndent())
        assertEquals("stored", putOutcome.trim('"'))

        val stored = session.appDataGet(app.record.appId, "items/e2e")!!.decodeToString()
        assertTrue(stored.contains("water"))

        val listed = waitForJs(host, """
            window.__rows ? String(window.__rows.length) :
              ((window.__listKicked = window.__listKicked || window.riot.list('items')
                .then(function (rows) { window.__rows = rows; })), null)
        """.trimIndent())
        assertEquals("1", listed.trim('"'))
    }

    @Test
    fun hostilePageCannotFetchOrEscapeScope() {
        val (_, _, host) = installTrustedChecklist()

        val fetchOutcome = waitForJs(host, """
            window.__fetch ? window.__fetch :
              fetch('https://example.com').then(
                function () { window.__fetch = 'FETCHED'; },
                function () { window.__fetch = 'blocked'; }) && null
        """.trimIndent())
        assertEquals("blocked", fetchOutcome.trim('"'))

        val escapeOutcome = waitForJs(host, """
            window.__esc ? window.__esc :
              window.riot.put('../escape', {x: 1}).then(
                function () { window.__esc = 'WROTE'; },
                function () { window.__esc = 'rejected'; }) && null
        """.trimIndent())
        assertEquals("rejected", escapeOutcome.trim('"'))
    }

    @Test
    fun untrustedAppNeverPassesTheLaunchGate() {
        val context = InstrumentationRegistry.getInstrumentation().context
        val profile = openLocalProfile()
        profile.createPublicSpace("Berlin Mutual Aid")
        val apps = RiotAppsController(profile.appRuntime())
        val app = apps.install(
            ChecklistFixture.manifestBytes(context),
            ChecklistFixture.bundleBytes(context),
        )
        assertThrows(IllegalStateException::class.java) { apps.requireTrusted(app) }
    }
}
```

(Adjust the JS-polling idioms if flaky — the pattern to preserve is: kick the promise once, stash its outcome on `window`, poll the stash. Match the generated binding names against `build/generated/riot-ffi/uniffi/` — e.g. whether `createPublicSpace` returns a value and whether the error type is a `MobileException` subclass — before assuming.)

- [ ] **Step 4: Run the instrumented suite**

From the repo root, then `apps/android/`, with an API 36 emulator running:

```sh
scripts/conference/build-native-core.sh
cd apps/android
JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew connectedDebugAndroidTest
```
Expected: all green — the 3 new `AppRuntimeEndToEndTest` tests plus the existing instrumented suite (`BindingSemanticsTest`, `FiveSurfaceSmokeTest`, `ProfileStoreHardeningTest`, `AndroidNearbyPlatformTest`). If `installApp` rejects the fixture bytes, the Kotlin canonical encoding has drifted from `manifest.rs`/`bundle.rs` — fix the encoder against those files; do not touch Rust.

- [ ] **Step 5: Full verification sweep + COLLABORATION.md + commit**

```sh
JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest connectedDebugAndroidTest assembleDebug
```
Expected: all green, APK assembles. Update this workstream's claim row in `COLLABORATION.md` (state, commits, exact commands + results per the file's ground rules; note Tasks 5–7 remain gated).

```bash
git add apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/ apps/android/app/build.gradle.kts COLLABORATION.md
git commit -m "test(android): checklist end-to-end instrumented test through the real WebView"
```

---

### Task 5: Persistence replay (GATED — do not start until the FFI lands)

**Gated on:** the iOS runtime plan's Task 5 landing the replay-persistence returns in `apps_ffi.rs`/`mobile_state.rs` (`trust_app`/`app_data_put` returning committed bundle bytes) — tracked in the `COLLABORATION.md` row "Active claim: JS apps runtime — iOS" (its "additive `apps_ffi.rs`/`mobile_state.rs` methods… persistence returns" item, itself gated on directory Task 6). Re-claim in `COLLABORATION.md` and re-read the landed signatures before starting; the shapes below are expectations, not ground truth.

**Files:** `PersistedProfile.kt` + `PersistedProfileCodec` (new versioned fields: installed app manifest+bundle byte pairs; replayable app bundles), `RiotController.kt` (`restore()` replays app bundles through the existing `inspectBytes → createPlan → accept` path exactly as alert bundles do today, then re-installs held apps via `install_app` — idempotent and entry-free), `RiotAppsController.kt`, tests mirroring `PersistedSyncImportTest`.

Approach is fixed by the spec: persist signed bundle bytes and replay — never re-call `trust_app`/`app_data_put` on restore (that would mint fresh signed entries and diverge across synced devices). Mind the codec's existing size ceilings and zeroization discipline (`TemporaryKey`, bounded reads) when extending `PersistedProfile`.

Commit: `feat(android): persist and replay app trust and app data across relaunch`

---

### Task 6: Starter catalog + listings consumption (GATED)

**Gated on:** directory plan Task 5 (`apps/starter.rs`) + Task 6 (`directory_listings` in `apps_ffi.rs`), and the iOS plan's Tasks 2–3 (packed artifacts + catalog fill) and Task 5 (`app_resource`) — check both claim rows in `COLLABORATION.md` and re-claim before starting.

**Files:** `RiotAppsController.kt`, `MainActivity.kt`, `AppResourceResolver.kt`, tests.

When landed: list the space's apps from the FFI listing surface instead of the session-local `InstalledAppsStore` (the starter checklist then appears with no install step — the two-step picker stops being the checklist's arrival path); serve resources through `app_resource` and delete the in-memory decoded-bundle retention (the Task 1 codec's decode path shrinks to test use). Do not build any listing/packing/starter surface in Rust — Android consumes only.

Commit: `feat(android): consume starter catalog listings and FFI resource serving`

---

### Task 7: Switch tests to the committed packed fixtures (GATED)

**Gated on:** directory plan Task 7 (`riot-app` CLI) + iOS plan Task 2 committing `fixtures/apps/checklist.manifest.cbor` / `checklist.bundle.cbor` as frozen artifacts — check `COLLABORATION.md`.

**Files:** `androidTest/.../apps/ChecklistFixture.kt`, `apps/android/app/build.gradle.kts` (assets dir already covers `fixtures/apps/`).

When landed: `AppRuntimeEndToEndTest` installs the committed `.cbor` artifacts directly (byte-identical app across iOS and Android — same `app_id`, same trust story), and the Kotlin manifest/bundle encoders are retired to adversarial-input duty only. Until then the Task 4 packer stands, with `install_app` as its canonicality oracle.

Commit: `test(android): install the committed checklist artifacts in the end-to-end test`

---

## After this plan lands

Non-gated end state: install-from-bytes → review → trust → run, fully proven on the emulator against today's FFI. Known follow-ups tracked, not dropped: Tasks 5–7 gates above; sync-completion `watch` trigger (blocked on FFI sync review being alert-only — platform handoff, deferred item 1; wire `SyncCoordinator`'s completion callback to `notifyDataChanged()` when app data rides sync); real display names replacing the `member-<hex8>` placeholder; revocation UI (storefront round).
