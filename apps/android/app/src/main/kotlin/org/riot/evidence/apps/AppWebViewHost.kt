package org.riot.evidence.apps

import android.annotation.SuppressLint
import android.content.Context
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.ServiceWorkerClientCompat
import androidx.webkit.ServiceWorkerControllerCompat
import androidx.webkit.WebSettingsCompat
import androidx.webkit.WebViewFeature
import java.io.ByteArrayInputStream

/**
 * A hardened WebView that serves an app's bundle from memory over a
 * synthetic per-app https origin via `shouldInterceptRequest`. Every
 * response carries the iOS CSP, and `blockNetworkLoads` is a second wall:
 * even a missed interception cannot reach the network.
 *
 * Task 1 covers hardening + interception only; the `window.riot` bridge and
 * document-start shim arrive in Task 2.
 */
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
            setGeolocationEnabled(false)
            allowUniversalAccessFromFileURLs = false
            allowFileAccessFromFileURLs = false
        }
        // Safe Browsing can send navigated URLs to Google out-of-band, a
        // channel blockNetworkLoads does not clearly cover. Disable it (the
        // manifest meta-data does the same, belt and suspenders).
        if (WebViewFeature.isFeatureSupported(WebViewFeature.SAFE_BROWSING_ENABLE)) {
            WebSettingsCompat.setSafeBrowsingEnabled(webView.settings, false)
        }
        // Defense-in-depth: deny every service-worker fetch. Interception +
        // blockNetworkLoads already contain workers, but this closes the
        // channel outright where the feature is available.
        if (WebViewFeature.isFeatureSupported(WebViewFeature.SERVICE_WORKER_BASIC_USAGE) &&
            WebViewFeature.isFeatureSupported(WebViewFeature.SERVICE_WORKER_SHOULD_INTERCEPT_REQUEST)
        ) {
            ServiceWorkerControllerCompat.getInstance().setServiceWorkerClient(
                object : ServiceWorkerClientCompat() {
                    override fun shouldInterceptRequest(
                        request: WebResourceRequest,
                    ): WebResourceResponse = WebResourceResponse(
                        "text/plain", null, 404, "Not Found",
                        mapOf("Content-Security-Policy" to CSP),
                        ByteArrayInputStream(ByteArray(0)),
                    )
                },
            )
        }
        webView.webViewClient = AppWebViewClient()
    }

    /** Named so Task 4 can drive interception through a real WebView. */
    inner class AppWebViewClient : WebViewClient() {
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
        ): Boolean {
            val uri = request.url
            return uri.scheme != "https" ||
                uri.host != resolver.originHost ||
                uri.port != -1
        }
    }

    fun load() = webView.loadUrl(resolver.entryUrl)

    fun destroy() = webView.destroy()
}
