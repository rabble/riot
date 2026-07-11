package org.riot.evidence

import android.content.Intent
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class FiveSurfaceSmokeTest {
    @Test
    @Suppress("DEPRECATION")
    fun launchExposesAllFiveConferenceSurfaces() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val intent = Intent(instrumentation.targetContext, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val activity = instrumentation.startActivitySync(intent)
        val labels = activity.window.decorView.allText().toSet()

        ConferenceSurface.entries.forEach { surface ->
            assertTrue("missing ${surface.label}", surface.label in labels)
        }
        val actionBarId = activity.resources.getIdentifier("action_bar_container", "id", "android")
        assertTrue("platform action bar must not cover Riot navigation", activity.findViewById<View?>(actionBarId) == null)
        val header = activity.window.decorView.findText("RIOT / PUBLIC INCIDENT SPACE")!!
        val location = IntArray(2).also(header::getLocationOnScreen)
        val statusBarInset = activity.window.decorView.rootWindowInsets.systemWindowInsetTop
        assertTrue("Riot header must clear the status bar", location[1] >= statusBarInset)

        activity.finish()
    }

    private fun View.allText(): List<String> = when (this) {
        is TextView -> listOf(text.toString())
        is ViewGroup -> (0 until childCount).flatMap { getChildAt(it).allText() }
        else -> emptyList()
    }

    private fun View.findText(value: String): TextView? = when (this) {
        is TextView -> takeIf { text.toString() == value }
        is ViewGroup -> (0 until childCount).firstNotNullOfOrNull { getChildAt(it).findText(value) }
        else -> null
    }
}
