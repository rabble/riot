package net.protest.riot.design

import androidx.activity.ComponentActivity
import android.os.Bundle
import androidx.activity.compose.setContent

/**
 * DEBUG-ONLY entry point for the design-system reference screen.
 *
 * Lives in `src/debug` so it is compiled out of release builds entirely — a
 * showcase must never be reachable in a shipped app. Launch it with:
 *
 *   adb shell am start -n net.protest.riot/net.protest.riot.design.RiotShowcaseActivity
 */
class RiotShowcaseActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { RiotTheme { RiotShowcase() } }
    }
}
