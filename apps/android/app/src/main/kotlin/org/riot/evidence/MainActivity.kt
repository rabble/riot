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
import org.riot.evidence.apps.UniffiProfilePort
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
        apps = RiotAppsController(
            controller.openAppRuntime(),
            onInstalled = controller::onAppInstalled,
            onTrusted = controller::onAppTrusted,
            onUntrusted = controller::onAppUntrusted,
        )
        // Re-admit apps persisted across a relaunch (install + serving-decode +
        // trust) before anything reads the installed list. App data was already
        // replayed into the store by RiotController's restore.
        apps.restore(controller.installedAppsSnapshot())
        directory = DirectoryController(
            // The subspace id is read fresh on each ask, not captured: joining a
            // space regenerates this profile's author, and a stale id would have
            // the directory looking for the wrong person's recommendations.
            UniffiDirectoryPort(controller.openAppRuntime()) {
                controller.profileSession().whoami().id
            },
            apps,
            AssetStarterCatalog(assets),
        )
        nearby = AndroidNearbyController(
            this,
            onChanged = {
                if (currentSurface == ConferenceSurface.CONNECTION) show(ConferenceSurface.CONNECTION)
            },
            onConnected = { phone, connection, incoming ->
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
                // EXACTLY ONE peer opens the protocol: the outgoing (dialing)
                // peer calls start(), the incoming (answering) peer calls
                // answer(). The core's ReconcileSession accepts a Hello only
                // from an idle session; two initiators fail each other.
                if (incoming) active.answer() else active.start()
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
            ConferenceSurface.NEWSWIRE -> showNewswire()
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
        // Turning an app on or off is the organizer's call, so the revoke
        // affordance is gated the same way as the approve one.
        val canApprove = runCatching { apps.isOrganizer() }.getOrDefault(false)
        apps.apps().forEach { app ->
            if (apps.isTrusted(app)) {
                content.addView(action("Open ${app.record.name}") { openApp(app) })
                if (canApprove) {
                    content.addView(action("Turn off ${app.record.name}") {
                        runAction("Turned off ${app.record.name}") {
                            apps.untrust(app)
                            show(ConferenceSurface.SPACES)
                        }
                    })
                }
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

            if (listing.permissions.isNotEmpty()) {
                content.addView(body("This app can:"))
                listing.permissions.forEach { content.addView(body("• $it")) }
            }

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
                // An app a neighbour carried in. Taking it up is the last hop
                // of discovery: it lands untrusted like any other, so it goes
                // to review rather than straight to opening.
                directory.canGet(listing) ->
                    content.addView(action("Get ${listing.name}") {
                        runAction("You have ${listing.name}") {
                            showAppReview(directory.get(listing))
                        }
                    })
                else ->
                    content.addView(body("Still arriving from your group…"))
            }

            // Endorsement is an organizer speaking for a space that already
            // trusts the app (design spec), so Recommend only appears once the
            // app is on in the current space. A row this person already
            // recommended offers the take-back instead — the two are exclusive,
            // and the controller, not this branch, decides which.
            if (directory.canRetract(listing)) {
                content.addView(action("Take back recommendation") {
                    runAction("Took back recommendation of ${listing.name}") {
                        directory.retractRecommendation(listing)
                        show(ConferenceSurface.APP_DIRECTORY)
                    }
                })
            } else if (directory.canRecommend(listing, space)) {
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
            }

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
            // Open the gated execution session (Unit 0C): this IS the launch gate
            // and it captures the approval generation + namespace, so a later
            // revoke / re-approval / namespace swap fails the running app's reads.
            val bridge = RiotJsBridge(
                UniffiAppDataPort(
                    controller.openAppExecution(gated.record.appId),
                    onCommitted = { key, bundle ->
                        controller.onAppDataCommitted(gated.record.appId, key, bundle)
                    },
                ),
                UniffiProfilePort(controller.profileSession()),
            )
            // §4.7: if a read/commit fails because access was invalidated
            // mid-use, close the app to its named destination rather than looping
            // against a dead session. The bridge fires this on the JS thread.
            bridge.onInvalidated = { runOnUiThread { closeApp() } }
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

    /**
     * The community newswire: the collective's published wire for the active
     * community, shown exactly as core's signed, already-split projection presents
     * it (front page / open wire), honoring editorial treatment (hidden and
     * tombstoned posts are redacted to their interstitial). Communal replies are
     * threaded under each post with a reply affordance; unread lands in a later
     * PR. An unavailable/stale projection degrades to the offline-stale copy
     * rather than crashing.
     */
    private fun showNewswire() {
        val community = controller.activeCommunity()
        if (community == null) {
            content.addView(body(
                "Join or create a community to see its newswire. " +
                    "Everything you already hold stays available offline.",
            ))
            content.addView(action("Go to spaces") { show(ConferenceSurface.SPACES) })
            return
        }
        content.addView(body(
            "Wire for ${community.title}. Reports appear exactly as the " +
                "collective's signed records present them.",
        ))
        val descriptor = community.descriptorEntryId
        renderSurface(NewswireScreen.resolve(descriptor) { controller.projectNewswire(it) }, descriptor)
    }

    private fun renderSurface(surface: NewswireSurface, descriptor: String?) {
        when (val wire = surface.wire) {
            NewswireWireState.OfflineStale -> {
                content.addView(heading(NewswireWireCopy.OFFLINE_TITLE))
                content.addView(body(NewswireWireCopy.OFFLINE_MESSAGE))
                content.addView(action("Try again") { show(ConferenceSurface.NEWSWIRE) })
            }
            NewswireWireState.EmptyWire -> {
                content.addView(heading(NewswireWireCopy.EMPTY_TITLE))
                content.addView(body(NewswireWireCopy.EMPTY_MESSAGE))
            }
            is NewswireWireState.PostsButNoFeature -> {
                content.addView(heading(NewswireWireCopy.NO_FEATURE_TITLE))
                content.addView(body(NewswireWireCopy.NO_FEATURE_MESSAGE))
                content.addView(heading(NewswireWireCopy.NO_FEATURE_LINK))
                wire.openWire.forEach { renderPost(it, surface, descriptor) }
            }
            is NewswireWireState.Featured -> {
                content.addView(heading("Featured"))
                wire.frontPage.forEach { renderPost(it, surface, descriptor) }
                content.addView(heading("Open wire"))
                wire.openWire.forEach { renderPost(it, surface, descriptor) }
            }
        }
    }

    /** A post row, its communal replies threaded beneath it, and — for an ordinary
     *  post on a projectable wire — a reply affordance. A redacted post shows no
     *  reply control (you reply to visible reports, not withheld ones). */
    private fun renderPost(row: NewswirePostRow, surface: NewswireSurface, descriptor: String?) {
        content.addView(postView(row))
        surface.comments(row.id).forEach { content.addView(commentView(it)) }
        if (row.display == NewswirePostDisplay.ORDINARY && descriptor != null) {
            addReplyAffordance(descriptor, row.id)
        }
    }

    /** One reply, indented under its parent. Hidden/tombstoned replies show only
     *  their signed interstitial — never the withheld words — like a post. */
    private fun commentView(comment: NewswireCommentRow): TextView {
        val text = when (comment.display) {
            NewswirePostDisplay.HIDDEN_INTERSTITIAL -> NewswireTreatmentCopy.HIDDEN_BODY
            NewswirePostDisplay.TOMBSTONED -> NewswireTreatmentCopy.TOMBSTONE_BODY
            NewswirePostDisplay.ORDINARY -> comment.body ?: ""
        }
        return body("↳ ${comment.author}\n$text").apply { setPadding(48, 8, 0, 8) }
    }

    private fun addReplyAffordance(descriptorEntryId: String, parentEntryId: String) {
        val input = EditText(this).apply { hint = "Reply to the collective" }
        content.addView(input)
        content.addView(action("Post reply") {
            val text = input.text.toString()
            if (!NewswireCommentValidator.isSubmittable(text)) {
                status.text = "A reply can't be empty."
                return@action
            }
            runAction("Reply signed and posted") {
                controller.createNewswireComment(descriptorEntryId, parentEntryId, text)
                show(ConferenceSurface.NEWSWIRE)
            }
        })
    }

    /** One post row. A hidden or tombstoned post shows only its signed
     *  interstitial — never the withheld headline — matching the treatment copy. */
    private fun postView(row: NewswirePostRow): TextView = when (row.display) {
        NewswirePostDisplay.HIDDEN_INTERSTITIAL ->
            body("${NewswireTreatmentCopy.HIDDEN_TITLE}\n${NewswireTreatmentCopy.HIDDEN_BODY}")
        NewswirePostDisplay.TOMBSTONED ->
            body("${NewswireTreatmentCopy.TOMBSTONE_TITLE}\n${NewswireTreatmentCopy.TOMBSTONE_BODY}")
        NewswirePostDisplay.ORDINARY -> body(ordinaryPostText(row))
    }

    private fun ordinaryPostText(row: NewswirePostRow): String {
        val badges = buildList {
            if (row.verificationCount > 0) add("Verified x${row.verificationCount}")
            if (row.hasCorrection) add(EditorialCorrectionLabel.TEXT)
            if (row.aiAssisted) add("AI-assisted")
        }
        return buildString {
            append(row.headline ?: "(untitled report)")
            append("\n")
            append(row.author)
            if (badges.isNotEmpty()) {
                append("\n")
                append(badges.joinToString(" • "))
            }
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
        // Second picker cancelled: drop the half-finished manifest so it can't
        // pair with an unrelated bundle on a later add.
        if (requestCode == PICK_APP_BUNDLE && resultCode != RESULT_OK) {
            pendingAppManifest = null
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
