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
import org.riot.evidence.transport.AndroidNearbyController
import org.riot.evidence.transport.NearbyUiState

class MainActivity : Activity() {
    private lateinit var controller: RiotController
    private lateinit var nearby: AndroidNearbyController
    private lateinit var content: LinearLayout
    private lateinit var status: TextView
    private var reviewedDraft: ReviewSnapshot? = null
    private var pendingImportEntries: List<CurrentEntry> = emptyList()
    private var currentSurface = ConferenceSurface.SPACES

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        controller = RiotController(filesDir)
        nearby = AndroidNearbyController(
            this,
            onChanged = {
                if (currentSurface == ConferenceSurface.CONNECTION) show(ConferenceSurface.CONNECTION)
            },
            onConnected = { _, _ ->
                // SyncCoordinator attaches here once the generated mobile sync bridge lands.
                status.text = "Connected nearby"
            },
        )
        setContentView(buildShell())
        show(ConferenceSurface.SPACES)
    }

    override fun onDestroy() {
        nearby.close()
        controller.close()
        super.onDestroy()
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
        currentSurface = surface
        content.removeAllViews()
        content.addView(heading(surface.label))
        when (surface) {
            ConferenceSurface.SPACES -> showSpaces()
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
    }

    private fun showConnection() {
        content.addView(body(nearby.state.message))
        if (nearby.state is NearbyUiState.Idle || nearby.state is NearbyUiState.Failed) {
            content.addView(action("Find nearby") { nearby.findNearby() })
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
    }
}
