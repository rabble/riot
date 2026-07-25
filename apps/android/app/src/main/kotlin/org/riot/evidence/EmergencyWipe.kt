package org.riot.evidence

import java.io.File

/**
 * Emergency wipe: destroy this device's copy of the community and the identity
 * that signs for it.
 *
 * Same shape as the iOS engine, for the same reason. Everything on disk is
 * either encrypted under, or meaningless without, the 32-byte wrapping key, so
 * destroying that key is an instant crypto-erase — the sealed identity becomes
 * unrecoverable the moment [arm] returns, however large the database is.
 *
 * The undo window therefore costs nothing in safety. The retained key lives
 * ONLY in memory during it, never rewritten to disk, so a process death
 * mid-window (an adversary force-stopping the app, a battery pull) leaves the
 * key gone and the data unrecoverable. AN INTERRUPTED WIPE FAILS TOWARD WIPED,
 * NEVER RESTORED — the only correct direction for a duress feature.
 *
 * [commit] then removes the files. That is cleanup, not the security boundary.
 */
class EmergencyWipe(
    private val wrappingKeys: SecretStore,
    private val databasePath: String,
    private val profileFile: File,
) {
    /** Held for the undo window. Memory only — writing it would defeat the erase. */
    private var undoKey: ByteArray? = null

    var isArmed = false
        private set

    /**
     * Every file the wipe removes. SQLite is not one file: the `-wal` holds the
     * most RECENT entries, so removing only the database would leave the newest
     * content on disk.
     */
    private val targets: List<File>
        get() =
            listOf("", "-wal", "-shm", "-journal").map { File(databasePath + it) } + profileFile

    /** Destroys the wrapping key now. Files wait for [commit]. */
    fun arm() {
        if (isArmed) return
        undoKey = wrappingKeys.read()
        wrappingKeys.clear()
        isArmed = true
    }

    /** Cancels an accidental trigger by putting the key back. */
    fun undo() {
        if (!isArmed) return
        val key = undoKey
        if (key != null) {
            wrappingKeys.write(key)
            key.fill(0)
        }
        undoKey = null
        isArmed = false
    }

    /**
     * Removes every target, then drops the retained key so undo is no longer
     * possible. Deletion continues past a failure so one undeletable file cannot
     * strand the rest.
     */
    fun commit() {
        check(isArmed) { "commit without arm would delete files while the key still exists" }
        targets.forEach { file -> runCatching { if (file.exists()) file.delete() } }
        undoKey?.fill(0)
        undoKey = null
        isArmed = false
    }
}
