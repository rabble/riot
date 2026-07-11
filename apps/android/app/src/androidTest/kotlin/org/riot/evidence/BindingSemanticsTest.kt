package org.riot.evidence

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.assertThrows
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.riot_ffi.AlertCertainty
import uniffi.riot_ffi.AlertDraftInput
import uniffi.riot_ffi.AlertSeverity
import uniffi.riot_ffi.AlertUrgency
import uniffi.riot_ffi.openLocalProfile

@RunWith(AndroidJUnit4::class)
class BindingSemanticsTest {
    @Test
    fun controllerCreatesAndSignsAReviewedAlert() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val directory = File(context.cacheDir, "controller-sign-${System.nanoTime()}").apply { mkdirs() }

        RiotController(directory).use { controller ->
            controller.createSpace("Controller Mutual Aid")
            val entry = controller.createAndSignAlert(
                "Local water point",
                "Bring a clean container.",
                aiAssisted = true,
            )
            assertEquals(entry.entryId, controller.entries().single().entryId)
            assertTrue(entry.aiAssisted)
        }

        directory.deleteRecursively()
    }

    @Test
    fun signedAlertSurvivesEncryptedReloadWithoutNetwork() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val file = File(context.filesDir, "binding-semantics-${System.nanoTime()}.bin")
        val store = AndroidKeystoreProfileStore("riot-binding-semantics", file)
        val now = System.currentTimeMillis().toULong() / 1_000UL

        val firstProfile = openLocalProfile()
        val space = firstProfile.createPublicSpace("Berlin Mutual Aid")
        val draft = firstProfile.createDraftAlert(
            AlertDraftInput(
                validFrom = now,
                expiresAt = now + 3_600UL,
                language = "en",
                urgency = AlertUrgency.IMMEDIATE,
                severity = AlertSeverity.SEVERE,
                certainty = AlertCertainty.OBSERVED,
                headline = "Water available at the courtyard",
                description = "Bring a clean container.",
                affectedAreaClaim = "Local-First Conf",
                sourceClaims = listOf("organizer desk"),
                aiAssisted = true,
            ),
        )
        val signed = firstProfile.signDraft(draft.draftId)
        val original = signed.entry
        store.save(
            PersistedProfile(
                PersistedSpace(space.namespaceId, space.title),
                listOf(original.toPersisted(signed.bundleBytes)),
            ),
        )
        firstProfile.close()

        val saved = store.load()!!
        val secondProfile = openLocalProfile()
        secondProfile.joinPublicSpace(saved.space.toPublicSpace())
        saved.alerts.forEach { alert ->
            val preview = secondProfile.inspectBytes(alert.bundleBytes, "android://encrypted-reload")
            val eligibleIds = preview.eligibleEntries().map { it.entryId }
            preview.createPlan(eligibleIds).accept()
        }
        val restored = secondProfile.listCurrentEntries().single()

        assertEquals(original.entryId, restored.entryId)
        assertEquals(64, restored.entryId.length)
        assertEquals(original.namespaceId, restored.namespaceId)
        assertEquals(original.signerId, restored.signerId)
        assertEquals(64, restored.signerId.length)
        assertEquals(original.freshness, restored.freshness)
        assertTrue(restored.aiAssisted)

        secondProfile.close()
        store.clear()
    }

    @Test
    fun acceptedImportSurvivesFreshControllerReload() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val directory = File(context.cacheDir, "import-reload-${System.nanoTime()}").apply { mkdirs() }
        val now = System.currentTimeMillis().toULong() / 1_000UL
        val sender = openLocalProfile()
        val space = sender.createPublicSpace("Imported Mutual Aid")
        val draft = sender.createDraftAlert(
            AlertDraftInput(
                validFrom = now,
                expiresAt = now + 3_600UL,
                language = "en",
                urgency = AlertUrgency.EXPECTED,
                severity = AlertSeverity.MODERATE,
                certainty = AlertCertainty.LIKELY,
                headline = "Imported water point",
                description = "Courtyard tap is working.",
                affectedAreaClaim = null,
                sourceClaims = listOf("conference test sender"),
                aiAssisted = false,
            ),
        )
        val signed = sender.signDraft(draft.draftId)

        RiotController(directory).use { receiver ->
            receiver.joinSpace(space)
            assertEquals(listOf(signed.entry.entryId), receiver.previewImport(signed.bundleBytes).map { it.entryId })
            receiver.acceptPreview()
        }
        RiotController(directory).use { restored ->
            assertEquals(listOf(signed.entry.entryId), restored.entries().map { it.entryId })
        }

        sender.close()
        directory.deleteRecursively()
    }

    @Test
    fun oversizedSavePreservesThePreviousEncryptedProfile() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val file = File(context.cacheDir, "bounded-store-${System.nanoTime()}.bin")
        val store = AndroidKeystoreProfileStore("riot-bounded-${System.nanoTime()}", file)
        val original = PersistedProfile(PersistedSpace("02".repeat(32), "Original"), emptyList())
        store.save(original)
        val oversized = PersistedProfile(
            original.space,
            List(3) { index ->
                PersistedAlert(
                    entryId = "%02x".format(index).repeat(32),
                    namespaceId = original.space.namespaceId,
                    signerId = "03".repeat(32),
                    headline = "Oversized $index",
                    createdAt = 1,
                    validFrom = null,
                    expiresAt = 2,
                    aiAssisted = false,
                    bundleBytes = ByteArray(2 * 1024 * 1024),
                )
            },
        )

        assertThrows(IllegalArgumentException::class.java) { store.save(oversized) }
        assertEquals(original, store.load())
        store.clear()
    }
}
