package org.riot.evidence

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.ByteArrayOutputStream
import java.io.DataOutputStream
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.SecretKey
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
    fun encryptedLegacyVersionOneMigratesThenKeepsSigner() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val directory = File(context.cacheDir, "identity-migration-${System.nanoTime()}").apply { mkdirs() }
        val store = AndroidKeystoreProfileStore(
            "riot-conference-profile",
            File(directory, "conference-profile.bin"),
        )
        val legacyProfile = PersistedProfile(PersistedSpace("02".repeat(32), "Legacy space"), emptyList())
        store.save(legacyProfile)
        writeEncryptedLegacyV1(
            File(directory, "conference-profile.bin"),
            "riot-conference-profile",
            legacyProfile,
        )
        assertEquals(null, store.load()!!.identityState)

        lateinit var migratedEntryId: String
        val migratedSigner = RiotController(directory).use {
            migratedEntryId = it.createAndSignAlert("Migrated", "Encrypted v1 content.", false).entryId
            it.identity().signingKeyId
        }
        val restoredSigner = RiotController(directory).use {
            assertEquals(listOf(migratedEntryId), it.entries().map { entry -> entry.entryId })
            it.identity().signingKeyId
        }
        val migratedState = store.load()!!.identityState!!

        assertEquals(migratedSigner, restoredSigner)
        assertEquals(PersistedProfileCodec.WRAPPING_KEY_BYTES, migratedState.wrappingKey.size)
        assertEquals(PersistedProfileCodec.SEALED_IDENTITY_BYTES, migratedState.sealedIdentity.size)
        migratedState.wrappingKey.fill(0)
        store.clear()
        directory.deleteRecursively()
    }

    @Test
    fun signerIdentitySurvivesFreshControllerAndCoreRestart() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val directory = File(context.cacheDir, "signer-continuity-${System.nanoTime()}").apply { mkdirs() }
        lateinit var firstSigner: String
        lateinit var firstEntryId: String

        RiotController(directory).use { first ->
            first.createSpace("Durable Mutual Aid")
            firstSigner = first.identity().signingKeyId
            val signed = first.createAndSignAlert("Before restart", "First offline post.", false)
            firstEntryId = signed.entryId
            assertEquals(firstSigner, signed.signerId)
        }
        RiotController(directory).use { restored ->
            val secondSigner = restored.identity().signingKeyId
            val signed = restored.createAndSignAlert("After restart", "Second offline post.", true)

            assertEquals(firstSigner, secondSigner)
            assertEquals(firstSigner, signed.signerId)
            assertEquals(64, secondSigner.length)
            assertEquals(setOf(firstEntryId, signed.entryId), restored.entries().map { it.entryId }.toSet())
        }

        directory.deleteRecursively()
    }

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

    private fun writeEncryptedLegacyV1(file: File, keyAlias: String, profile: PersistedProfile) {
        val plaintext = ByteArrayOutputStream().use { bytes ->
            DataOutputStream(bytes).use { output ->
                output.writeInt(0x52494f54)
                output.writeInt(1)
                output.writeLegacyString(profile.space.namespaceId)
                output.writeLegacyString(profile.space.title)
                output.writeInt(0)
            }
            bytes.toByteArray()
        }
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val key = keyStore.getKey(keyAlias, null) as SecretKey
        val cipher = Cipher.getInstance("AES/GCM/NoPadding").apply {
            init(Cipher.ENCRYPT_MODE, key)
        }
        val ciphertext = TemporaryKey.useOwned(plaintext) { cipher.doFinal(it) }
        val envelope = ByteArrayOutputStream().use { bytes ->
            DataOutputStream(bytes).use { output ->
                output.writeInt(cipher.iv.size)
                output.write(cipher.iv)
                output.writeInt(ciphertext.size)
                output.write(ciphertext)
            }
            bytes.toByteArray()
        }
        file.writeBytes(envelope)
    }

    private fun DataOutputStream.writeLegacyString(value: String) {
        val encoded = value.toByteArray(Charsets.UTF_8)
        writeInt(encoded.size)
        write(encoded)
    }
}
