package org.riot.evidence

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ReviewSnapshotTest {
    @Test
    fun signingRequiresHeadlineDescriptionAndDisclosureToMatchReview() {
        val reviewed = ReviewSnapshot("Water available", "Bring a container", aiAssisted = true)

        assertTrue(reviewed.matches("Water available", "Bring a container", aiAssisted = true))
        assertFalse(reviewed.matches("Different", "Bring a container", aiAssisted = true))
        assertFalse(reviewed.matches("Water available", "Different", aiAssisted = true))
        assertFalse(reviewed.matches("Water available", "Bring a container", aiAssisted = false))
    }
}
