package org.riot.evidence

import org.junit.Assert.assertThrows
import org.junit.Test

class ProfileFileBoundsTest {
    @Test
    fun rejectsOversizedFileAtMetadataBoundaryBeforeRead() {
        assertThrows(IllegalArgumentException::class.java) {
            AndroidKeystoreProfileStore.requireBoundedFileLengthForTest(4L * 1024 * 1024 + 65)
        }
    }
}
