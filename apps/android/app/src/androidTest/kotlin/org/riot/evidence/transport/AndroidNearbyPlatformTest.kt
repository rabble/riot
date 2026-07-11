package org.riot.evidence.transport

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AndroidNearbyPlatformTest {
    @Test
    fun runtimePermissionsFollowAndroidVersionBoundary() {
        assertEquals(
            setOf(Manifest.permission.ACCESS_FINE_LOCATION),
            NearbyPermissions.runtimePermissions(30).toSet(),
        )
        assertEquals(
            setOf(
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.BLUETOOTH_CONNECT,
                Manifest.permission.BLUETOOTH_ADVERTISE,
            ),
            NearbyPermissions.runtimePermissions(36).toSet(),
        )
    }

    @Test
    fun installedPackageExposesPermissionsForThisAndroidVersion() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val info = context.packageManager.getPackageInfo(
            context.packageName,
            PackageManager.GET_PERMISSIONS,
        )
        val declared = info.requestedPermissions.orEmpty().toSet()

        assertTrue(Manifest.permission.BLUETOOTH_SCAN in declared)
        assertTrue(Manifest.permission.BLUETOOTH_CONNECT in declared)
        assertTrue(Manifest.permission.BLUETOOTH_ADVERTISE in declared)
        if (Build.VERSION.SDK_INT <= 30) {
            assertTrue(Manifest.permission.ACCESS_FINE_LOCATION in declared)
        } else {
            assertFalse(Manifest.permission.ACCESS_FINE_LOCATION in declared)
        }
        assertTrue(Manifest.permission.INTERNET in declared) // Required by Android even for direct LAN sockets.
    }
}
