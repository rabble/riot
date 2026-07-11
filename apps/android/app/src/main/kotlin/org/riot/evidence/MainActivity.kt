package org.riot.evidence

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import uniffi.riot_ffi.CurrentEntry
import org.riot.evidence.apps.AppBundleCodec
import org.riot.evidence.apps.AppResourceResolver
import org.riot.evidence.apps.AppWebViewHost
import org.riot.evidence.apps.AssetStarterCatalog
import org.riot.evidence.apps.DirectoryController
import org.riot.evidence.apps.InstalledApp
import org.riot.evidence.apps.RiotAppsController
import org.riot.evidence.apps.UniffiDirectoryPort
import org.riot.evidence.apps.RiotJsBridge
import org.riot.evidence.apps.UniffiAppDataPort
import org.riot.evidence.transport.AndroidNearbyController
import org.riot.evidence.transport.NearbyUiState
import org.riot.evidence.transport.NearbyUiActions
import org.riot.evidence.transport.SyncCoordinator

class MainActivity : Activity() {
    private lateinit var controller: RiotController
    private lateinit var nearby: AndroidNearbyController
    private lateinit var content: LinearLayout
    private lateinit var status: TextView
    private var reviewedDraft: ReviewSnapshot? = null
    private var pendingImportEntries: List<CurrentEntry> = emptyList()
    private var currentSurface = ConferenceSurface.SPACES
    private var syncCoordinator: SyncCoordinator? = null
    private var syncState: NearbyUiState? = null
    private lateinit var apps: RiotAppsController
    private lateinit var directory: DirectoryController
    private var runningApp: Pair<InstalledApp, AppWebViewHost>? = null
    private var pendingAppManifest: ByteArray? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        controller = RiotController(filesDir)
        apps = RiotAppsController(controller.openAppRuntime())
        directory = DirectoryController(
            UniffiDirectoryPort(controller.openAppRuntime()),
            apps,
            AssetStarterCatalog(assets),
        )
        nearby = AndroidNearbyController(
            this,
            onChanged = {
                if (currentSurface == ConferenceSurface.CONNECTION) show(ConferenceSurface.CONNECTION)
            },
            onConnected = { phone, connection ->
                syncCoordinator?.close()
                lateinit var active: SyncCoordinator
                active = SyncCoordinator(
                    connection,
                    controller.openSyncBridge(),
                    phone.friendlyName,
                ) { next ->
                    runOnUiThread {
                        if (syncCoordinator === active) {
                            syncState = next
                            status.text = next.message
                            if (currentSurface == ConferenceSurface.CONNECTION) {
                                show(ConferenceSurface.CONNECTION)
                            }
                        }
                    }
                }
                syncCoordinator = active
                syncState = NearbyUiState.Connecting
                active.start()
            },
        )
        setContentView(buildShell())
        // Bring the built-in tools in before the first render so the checklist
        // is openable under Tools and in the directory out of the box. If its
        // shipped bytes are unreadable the directory still lists it (the core
        // compiles it in) — it just can't be opened, so a silent skip is fine.
        try {
            directory.ensureStarterInstalled()
        } catch (_: Exception) {
        }
        show(ConferenceSurface.SPACES)
    }

    override fun onDestroy() {
        runningApp?.second?.destroy()
        runningApp = null
        syncCoordinator?.close()
        nearby.close()
        controller.close()
        super.onDestroy()
    }

    override fun onResume() {
        super.onResume()
        // Foreground `watch` trigger (spec): re-fire app watchers on return.
        runningApp?.second?.notifyDataChanged()
    }

    @Suppress("DEPRECATION")
    private fun buildShell(): View = vertical().apply {
        setPadding(24, 32, 24, 24)
        setOnApplyWindowInsetsListener { view, insets ->
            view.setPadding(
                24,
                32 + insets.systemWindowInsetTop,
                24,
                24 + insets.systemWindowInsetBottom,
            )
            insets
        }
        addView(TextView(context).apply {
            text = "RIOT / PUBLIC INCIDENT SPACE"
            textSize = 22f
            setTypeface(typeface, Typeface.BOLD)
        })
        addView(TextView(context).apply {
            text = "Local-first • human-reviewed • public"
            textSize = 14f
        })
        addView(HorizontalScrollView(context).apply {
            addView(LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL
                ConferenceSurface.entries.forEach { surface ->
                    addView(Button(context).apply {
                        text = surface.label
                        isAllCaps = false
                        setOnClickListener { show(surface) }
                    })
                }
            })
        })
        content = vertical()
        addView(ScrollView(context).apply { addView(content) }, weighted())
        status = TextView(context).apply {
            text = "Offline ready"
            gravity = Gravity.CENTER_VERTICAL
        }
        addView(status)
    }

    private fun show(surface: ConferenceSurface) {
        if (runningApp != null) {
            runningApp?.second?.destroy()
            runningApp = null
        }
        currentSurface = surface
        content.removeAllViews()
        content.addView(heading(surface.label))
        when (surface) {
            ConferenceSurface.SPACES -> showSpaces()
            ConferenceSurface.APP_DIRECTORY -> showDirectory()
            ConferenceSurface.INCIDENT_BOARD -> showBoard()
            ConferenceSurface.COMPOSE_AND_SIGN -> showCompose()
            ConferenceSurface.IMPORT_PREVIEW -> showImportPreview()
            ConferenceSurface.CONNECTION -> showConnection()
        }
    }

    private fun showSpaces() {
        val current = controller.currentSpace
        content.addView(body(current?.let {
            "${it.title}\nPublic namespace\n${it.namespaceId}"
        } ?: "No public incident space yet."))
        val title = EditText(this).apply {
            hint = "Space title"
            setText(current?.title ?: "Berlin Mutual Aid")
        }
        content.addView(title)
        content.addView(action("Create public space") {
            runAction("Public space created") {
                controller.createSpace(title.text.toString())
                show(ConferenceSurface.SPACES)
            }
        })
        if (current != null) {
            showTools()
        }
    }

    private fun showTools() {
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
    }

    /**
     * The discovery surface: every app this profile can see — built-in,
     * shared into a space, or carried in by sync — with what it does, who
     * recommends it, and the actions to review, recommend, or pass it on.
     * Plain language only; the word "install" never appears.
     */
    private fun showDirectory() {
        val space = controller.currentSpace
        content.addView(body(
            "Every tool your communities carry shows up here. Nothing runs until " +
                "an organizer turns it on for a space.",
        ))
        val listings = directory.listings()
        if (listings.isEmpty()) {
            content.addView(body("No tools yet."))
            return
        }
        listings.forEach { listing ->
            val trusted = directory.trustedInCurrentSpace(listing, space)
            content.addView(heading("${listing.name} · ${listing.version}"))
            content.addView(body(listing.description))

            val badges = buildList {
                if (listing.builtIn) add("Built in")
                if (trusted) add("On in this space")
                if (!listing.bundlePresent) add("Still arriving from your group")
            }
            if (badges.isNotEmpty()) content.addView(body(badges.joinToString("  ·  ")))

            content.addView(body("This app can:"))
            listing.permissions.forEach { content.addView(body("• $it")) }

            val met = listing.endorsingMetSubspaces.size
            val unmet = listing.endorsingUnmetCount.toInt()
            if (met + unmet > 0) {
                val parts = buildList {
                    if (met > 0) {
                        add(if (met == 1) "1 group you've met" else "$met groups you've met")
                    }
                    if (unmet > 0) add("$unmet you haven't met")
                }
                content.addView(body("Recommended by ${parts.joinToString(", ")}"))
            }

            val installed = directory.installedFor(listing)
            when {
                installed != null && trusted ->
                    content.addView(action("Open ${listing.name}") { openApp(installed) })
                installed != null ->
                    content.addView(action("Review ${listing.name}") { showAppReview(installed) })
                trusted && !listing.bundlePresent ->
                    content.addView(body("Still arriving from your group…"))
            }

            val note = EditText(this).apply {
                hint = "Why you recommend it (optional)"
            }
            content.addView(note)
            content.addView(action("Recommend") {
                runAction("Recommended ${listing.name}") {
                    directory.recommend(listing, note.text.toString())
                    show(ConferenceSurface.APP_DIRECTORY)
                }
            })

            if (space != null) {
                content.addView(action("Share to this space") {
                    runAction("Shared ${listing.name} to ${space.title}") {
                        directory.share(listing, space)
                        show(ConferenceSurface.APP_DIRECTORY)
                    }
                })
            }
        }
    }

    /** The trust-decision moment: plain language only, mirroring iOS. */
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

    private fun openDocumentIntent() = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
        addCategory(Intent.CATEGORY_OPENABLE)
        type = "application/octet-stream"
    }

    private fun showBoard() {
        val entries = controller.entries()
        if (entries.isEmpty()) {
            content.addView(body("No alerts yet. Everything shown here is available offline."))
        }
        entries.forEach { entry ->
            content.addView(body(
                "${entry.headline}\n" +
                    "Signer: ${entry.signerId}\n" +
                    "Entry: ${entry.entryId}\n" +
                    "Fresh until: ${entry.freshness.expiresAt}\n" +
                    if (entry.aiAssisted) "AI-assisted draft • human signed" else "Human drafted and signed",
            ))
        }
    }

    private fun showCompose() {
        content.addView(body("Model output stays editable. Nothing publishes until you review and sign."))
        val headline = EditText(this).apply { hint = "Alert headline" }
        val description = EditText(this).apply {
            hint = "What should people know?"
            minLines = 3
        }
        val aiAssisted = CheckBox(this).apply { text = "This draft used model assistance" }
        val review = body("Not reviewed")
        content.addView(headline)
        content.addView(description)
        content.addView(aiAssisted)
        content.addView(action("Review draft") {
            reviewedDraft = ReviewSnapshot.capture(
                headline.text.toString(),
                description.text.toString(),
                aiAssisted.isChecked,
            )
            review.text = "Ready to sign locally:\n${reviewedDraft!!.headline}\n${reviewedDraft!!.description}\n" +
                if (reviewedDraft!!.aiAssisted) "AI-assisted draft" else "Human draft"
        })
        content.addView(review)
        content.addView(action("Sign and add to board") {
            runAction("Alert signed locally") {
                val reviewed = checkNotNull(reviewedDraft) { "Review the current draft first" }
                check(reviewed.headline.isNotEmpty() && reviewed.matches(
                    headline.text.toString(),
                    description.text.toString(),
                    aiAssisted.isChecked,
                )) {
                    "Review the current draft first"
                }
                controller.createAndSignAlert(
                    headline.text.toString(),
                    description.text.toString(),
                    aiAssisted.isChecked,
                )
                reviewedDraft = null
                show(ConferenceSurface.INCIDENT_BOARD)
            }
        })
    }

    private fun showImportPreview() {
        content.addView(body("Signed bundles are inspected first. You explicitly accept eligible entries."))
        content.addView(action("Choose signed Riot bundle") {
            startActivityForResult(
                Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "application/octet-stream"
                },
                IMPORT_DOCUMENT,
            )
        })
        pendingImportEntries.forEach { content.addView(body("Eligible: ${it.headline}\n${it.entryId}")) }
        content.addView(action("Accept previewed entries") {
            runAction("Import accepted") {
                check(pendingImportEntries.isNotEmpty()) { "Choose a bundle with eligible entries first" }
                controller.acceptPreview()
                pendingImportEntries = emptyList()
                show(ConferenceSurface.INCIDENT_BOARD)
            }
        })
    }

    @Deprecated("Kept intentionally small for the conference document picker")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == IMPORT_DOCUMENT && resultCode == RESULT_OK) {
            runAction("Bundle previewed") {
                val uri = checkNotNull(data?.data)
                val bytes = contentResolver.openInputStream(uri)!!.use {
                    BoundedInput.read(it, PersistedProfileCodec.MAX_ENCODED_BYTES)
                }
                pendingImportEntries = controller.previewImport(bytes)
                show(ConferenceSurface.IMPORT_PREVIEW)
            }
        }
        if (requestCode == PICK_APP_MANIFEST && resultCode == RESULT_OK) {
            runAction("Now choose the tool's bundle file") {
                val uri = checkNotNull(data?.data)
                pendingAppManifest = contentResolver.openInputStream(uri)!!.use {
                    BoundedInput.read(it, MAX_APP_MANIFEST_BYTES)
                }
                startActivityForResult(openDocumentIntent(), PICK_APP_BUNDLE)
            }
        }
        if (requestCode == PICK_APP_BUNDLE && resultCode == RESULT_OK) {
            runAction("Tool added — review it under Tools") {
                val manifest = checkNotNull(pendingAppManifest) { "That file isn't a Riot tool" }
                val uri = checkNotNull(data?.data)
                val bytes = contentResolver.openInputStream(uri)!!.use {
                    BoundedInput.read(it, AppBundleCodec.MAX_BUNDLE_TOTAL_BYTES)
                }
                try {
                    apps.install(manifest, bytes)
                } catch (error: Exception) {
                    throw IllegalStateException("That file isn't a Riot tool")
                } finally {
                    pendingAppManifest = null
                }
                show(ConferenceSurface.SPACES)
            }
        }
    }

    private fun showConnection() {
        val visibleState = syncState ?: nearby.state
        content.addView(body(visibleState.message))
        if (visibleState is NearbyUiState.UpdatesReady) {
            content.addView(action("Add them") { syncCoordinator?.acceptImport() })
            content.addView(action("Not now") { syncCoordinator?.rejectImport() })
        }
        if (NearbyUiActions.canFindAgain(visibleState)) {
            content.addView(action("Find nearby") {
                syncCoordinator?.close()
                syncCoordinator = null
                syncState = null
                nearby.findNearby()
            })
        }
        nearby.phones.forEach { phone ->
            content.addView(action(phone.friendlyName) { nearby.select(phone) })
        }
        if (nearby.state is NearbyUiState.ConfirmPairing) {
            content.addView(action("Confirm") { nearby.confirmPairing() })
            content.addView(action("Cancel") { nearby.cancelPairing() })
        }
        content.addView(body(
            "Works directly between nearby phones. It never switches to an internet service. " +
                "Physical-phone radio verification is still required.",
        ))
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == AndroidNearbyController.PERMISSION_REQUEST) {
            nearby.permissionResult(grantResults.isNotEmpty() && grantResults.all { it == android.content.pm.PackageManager.PERMISSION_GRANTED })
        }
    }

    private fun runAction(success: String, action: () -> Unit) {
        try {
            action()
            status.text = success
        } catch (error: Exception) {
            status.text = error.message ?: error.javaClass.simpleName
        }
    }

    private fun vertical() = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
    private fun heading(text: String) = TextView(this).apply {
        this.text = text
        textSize = 20f
        setTypeface(typeface, Typeface.BOLD)
        setPadding(0, 24, 0, 12)
    }
    private fun body(text: String) = TextView(this).apply {
        this.text = text
        textSize = 16f
        setPadding(0, 12, 0, 12)
        setTextIsSelectable(true)
    }
    private fun action(label: String, block: () -> Unit) = Button(this).apply {
        text = label
        isAllCaps = false
        setOnClickListener { block() }
    }
    private fun weighted() = LinearLayout.LayoutParams(
        LinearLayout.LayoutParams.MATCH_PARENT,
        0,
        1f,
    )

    private companion object {
        const val IMPORT_DOCUMENT = 10
        const val PICK_APP_MANIFEST = 11
        const val PICK_APP_BUNDLE = 12
        const val MAX_APP_MANIFEST_BYTES = 4_096
    }
}
