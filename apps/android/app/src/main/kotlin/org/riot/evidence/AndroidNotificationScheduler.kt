package org.riot.evidence

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build

/**
 * The production [NotificationScheduling]: posts real system notifications via the
 * platform [NotificationManager] on a dedicated channel. The thin,
 * Android-framework side of the notifier — the pure decision lives in
 * `LocalNotifier.kt` and is host-JVM tested; this effect layer needs a
 * device/emulator and is exercised by instrumented tests, mirroring iOS's
 * `UserNotificationScheduler`.
 *
 * Uses platform `android.app.*` APIs only (minSdk is 26, so channels and the
 * two-arg `Notification.Builder` are always present) — no androidx.core, so the
 * app's offline dependency graph is untouched.
 */
class AndroidNotificationScheduler(private val context: Context) : NotificationScheduling {
    private val manager: NotificationManager =
        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    init {
        ensureChannel()
    }

    override fun currentAuthorization(): NotifierAuthorization {
        // A disabled app/channel is an explicit denial regardless of API level.
        if (!manager.areNotificationsEnabled()) return NotifierAuthorization.DENIED
        // API 33+ additionally gates on the POST_NOTIFICATIONS runtime grant.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val granted = context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED
            // Not granted and not hard-disabled → the reader has not decided; the
            // go-live PR prompts through an Activity result launcher (a Context
            // cannot drive the permission dialog).
            return if (granted) NotifierAuthorization.AUTHORIZED else NotifierAuthorization.NOT_DETERMINED
        }
        return NotifierAuthorization.AUTHORIZED
    }

    override fun requestAuthorization(): NotifierAuthorization {
        // The POST_NOTIFICATIONS prompt is an Activity-scoped async flow, not
        // something a Context can drive synchronously; the shell owns it. Report
        // current state so the pure notifier's contract still holds.
        return currentAuthorization()
    }

    override fun schedule(identifier: String, title: String, body: String, communityId: String) {
        if (!manager.areNotificationsEnabled()) return
        val notification = Notification.Builder(context, CHANNEL_ID)
            // The app ships no resource drawables; a system icon keeps the effect
            // self-contained. The go-live PR can swap in a branded asset.
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle(title)
            .setContentText(body)
            // Groups this community's notifications; a future tap handler reads it.
            .setGroup(communityId)
            .setAutoCancel(true)
            .setCategory(Notification.CATEGORY_SOCIAL)
            .build()
        // A stable per-community tag means a newer batch REPLACES the older pending
        // one — one entry per community, not a stack that grows per post.
        try {
            manager.notify(identifier, NOTIFICATION_ID, notification)
        } catch (_: SecurityException) {
            // POST_NOTIFICATIONS revoked between the check and the post — drop it.
        }
    }

    private fun ensureChannel() {
        // minSdk is 26, so the channel API is always present.
        val channel = NotificationChannel(
            CHANNEL_ID,
            "New content",
            NotificationManager.IMPORTANCE_DEFAULT,
        ).apply { description = "New posts synced from nearby community members" }
        manager.createNotificationChannel(channel)
    }

    private companion object {
        const val CHANNEL_ID = "riot.newcontent"
        // The per-community tag differentiates entries; the numeric id is shared.
        const val NOTIFICATION_ID = 1
    }
}
