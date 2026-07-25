package net.protest.riot

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

private class WipeSecretStoreFake(private var secret: ByteArray? = ByteArray(32) { 7 }) : SecretStore {
    var clearCount = 0
        private set
    var restored: ByteArray? = null
        private set

    override fun read(): ByteArray? = secret?.copyOf()

    override fun write(secret: ByteArray) {
        restored = secret.copyOf()
        this.secret = secret.copyOf()
    }

    override fun clear() {
        clearCount += 1
        secret = null
    }
}

class EmergencyWipeTest {
    @get:Rule val folder = TemporaryFolder()

    private fun wipeWith(keys: WipeSecretStoreFake): Pair<EmergencyWipe, List<File>> {
        val db = folder.newFile("riot.db")
        val wal = folder.newFile("riot.db-wal")
        val shm = folder.newFile("riot.db-shm")
        val profile = folder.newFile("conference-profile.bin")
        val wipe =
            EmergencyWipe(
                wrappingKeys = keys,
                databasePath = db.absolutePath,
                profileFile = profile,
            )
        return wipe to listOf(db, wal, shm, profile)
    }

    /**
     * The security boundary: arming destroys the key immediately, so the sealed
     * identity is unrecoverable ciphertext before any file work begins.
     */
    @Test
    fun `arming destroys the key and leaves the files for the window`() {
        val keys = WipeSecretStoreFake()
        val (wipe, files) = wipeWith(keys)

        wipe.arm()

        assertEquals(1, keys.clearCount)
        assertTrue("files wait for the undo window", files.all { it.exists() })
    }

    /** The accidental trigger: undo puts the same key back and deletes nothing. */
    @Test
    fun `undo restores the key and keeps every file`() {
        val keys = WipeSecretStoreFake()
        val (wipe, files) = wipeWith(keys)
        val original = keys.read()

        wipe.arm()
        wipe.undo()

        assertArrayEqualsNullable(original, keys.read())
        assertTrue(files.all { it.exists() })
    }

    /**
     * Commit removes the database AND its sidecars — the `-wal` holds the newest
     * entries, so deleting only riot.db would leave the most recent content.
     */
    @Test
    fun `commit removes the database its sidecars and the profile`() {
        val keys = WipeSecretStoreFake()
        val (wipe, files) = wipeWith(keys)

        wipe.arm()
        wipe.commit()

        assertTrue("nothing may survive a committed wipe", files.none { it.exists() })
    }

    /** Once committed there is nothing to undo — the key must stay destroyed. */
    @Test
    fun `undo after commit does not restore the key`() {
        val keys = WipeSecretStoreFake()
        val (wipe, _) = wipeWith(keys)

        wipe.arm()
        wipe.commit()
        wipe.undo()

        assertEquals(null, keys.read())
    }

    /** Arming twice must not strand the undo by discarding the retained key. */
    @Test
    fun `arming twice keeps the undo working`() {
        val keys = WipeSecretStoreFake()
        val (wipe, _) = wipeWith(keys)
        val original = keys.read()

        wipe.arm()
        wipe.arm()
        wipe.undo()

        assertEquals(1, keys.clearCount)
        assertArrayEqualsNullable(original, keys.read())
    }

    /** Committing without arming is refused: it would delete files while the key still exists. */
    @Test
    fun `commit without arming is refused`() {
        val keys = WipeSecretStoreFake()
        val (wipe, files) = wipeWith(keys)

        assertThrows(IllegalStateException::class.java) { wipe.commit() }
        assertTrue(files.all { it.exists() })
    }

    /** A fresh install with no identity yet wipes harmlessly. */
    @Test
    fun `arming with no stored key is harmless`() {
        val keys = WipeSecretStoreFake(secret = null)
        val (wipe, files) = wipeWith(keys)

        wipe.arm()
        wipe.undo()
        assertFalse(files.none { it.exists() })
    }

    private fun assertArrayEqualsNullable(expected: ByteArray?, actual: ByteArray?) {
        if (expected == null) {
            assertEquals(null, actual)
        } else {
            org.junit.Assert.assertArrayEquals(expected, actual)
        }
    }
}
