package org.riot.evidence.apps

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.riot.evidence.AndroidKeystoreProfileStore
import org.riot.evidence.RiotController

/**
 * The gold standard for Task 5: an installed + trusted app and its app data
 * survive a full profile close/reopen. First a live `RiotController` installs,
 * trusts, and writes app data through the same callbacks `MainActivity` wires;
 * then a fresh `RiotController` over the same encrypted store must re-admit all
 * of it. Proves the persist-and-replay path end-to-end against the real FFI
 * and Android Keystore, not just the JVM codec.
 */
@RunWith(AndroidJUnit4::class)
class AppPersistenceRestartTest {
    @Test
    fun installedTrustedAppAndItsDataSurviveReopen() {
        val testAssets = InstrumentationRegistry.getInstrumentation().context
        val target = InstrumentationRegistry.getInstrumentation().targetContext
        val directory = File(target.cacheDir, "app-persist-restart-${System.nanoTime()}").apply { mkdirs() }

        var appId = ""
        try {
            RiotController(directory).use { controller ->
                controller.createSpace("Berlin Mutual Aid")
                val apps = RiotAppsController(
                    controller.openAppRuntime(),
                    onInstalled = controller::onAppInstalled,
                    onTrusted = controller::onAppTrusted,
                )
                val app = apps.install(
                    ChecklistFixture.manifestBytes(testAssets),
                    ChecklistFixture.bundleBytes(testAssets),
                )
                appId = app.record.appId
                apps.trust(app)
                val port = UniffiAppDataPort(
                    controller.openAppRuntime(),
                    appId,
                    onCommitted = { key, bundle -> controller.onAppDataCommitted(appId, key, bundle) },
                )
                port.put("items/water", """{"text":"water at courtyard","done":false}""".toByteArray())
            }

            RiotController(directory).use { controller ->
                val apps = RiotAppsController(
                    controller.openAppRuntime(),
                    onInstalled = controller::onAppInstalled,
                    onTrusted = controller::onAppTrusted,
                )
                apps.restore(controller.installedAppsSnapshot())

                val restored = apps.find(appId)
                assertNotNull("app must be re-installed after reopen", restored)
                assertTrue("trust must survive reopen", apps.isTrusted(restored!!))

                val value = UniffiAppDataPort(controller.openAppRuntime(), appId).get("items/water")
                assertNotNull("app data must survive reopen", value)
                assertTrue(value!!.decodeToString().contains("water at courtyard"))
            }
        } finally {
            AndroidKeystoreProfileStore(
                "riot-conference-profile",
                File(directory, "conference-profile.bin"),
            ).clear()
            directory.deleteRecursively()
        }
    }

    @Test
    fun untrustedAppStaysUntrustedAfterReopen() {
        val testAssets = InstrumentationRegistry.getInstrumentation().context
        val target = InstrumentationRegistry.getInstrumentation().targetContext
        val directory = File(target.cacheDir, "app-persist-untrusted-${System.nanoTime()}").apply { mkdirs() }

        var appId = ""
        try {
            RiotController(directory).use { controller ->
                controller.createSpace("Berlin Mutual Aid")
                val apps = RiotAppsController(
                    controller.openAppRuntime(),
                    onInstalled = controller::onAppInstalled,
                    onTrusted = controller::onAppTrusted,
                )
                val app = apps.install(
                    ChecklistFixture.manifestBytes(testAssets),
                    ChecklistFixture.bundleBytes(testAssets),
                )
                appId = app.record.appId
                // Deliberately no trust.
            }

            RiotController(directory).use { controller ->
                val apps = RiotAppsController(controller.openAppRuntime())
                apps.restore(controller.installedAppsSnapshot())
                val restored = apps.find(appId)
                assertNotNull("install must survive reopen even without trust", restored)
                assertFalse("an untrusted app must not become trusted", apps.isTrusted(restored!!))
            }
        } finally {
            AndroidKeystoreProfileStore(
                "riot-conference-profile",
                File(directory, "conference-profile.bin"),
            ).clear()
            directory.deleteRecursively()
        }
    }
}
