package org.riot.evidence

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/** WU0 preflight: proves the instrumentation toolchain runs on the pinned emulator. */
@RunWith(AndroidJUnit4::class)
class BlankInstrumentationTest {
    @Test
    fun instrumentationContextTargetsEvidencePackage() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        assertEquals("org.riot.evidence", context.packageName)
    }
}
