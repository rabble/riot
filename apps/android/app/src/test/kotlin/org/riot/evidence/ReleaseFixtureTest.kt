package org.riot.evidence

import java.security.MessageDigest
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class ReleaseFixtureTest {
    private val expectedDigest = "930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469"
    private val stateIds = listOf(
        "community-newswire",
        "signed-publishing",
        "labels-and-signatures",
        "community-tools",
        "nearby-exchange",
        "offline-copy",
    )

    @Test
    fun canonicalFixtureIsPinnedAndExact() {
        val bytes = fixtureBytes()
        assertEquals(expectedDigest, sha256(bytes))
        val fixture = ReleaseFixture.decode(bytes)
        assertEquals(1, fixture.schemaVersion)
        assertEquals("riot-1.0-synthetic-v1", fixture.fixtureRevision)
        assertEquals("synthetic", fixture.fixtureKind)
        assertEquals("2026-07-24T12:00:00.000Z", fixture.fixedClock)
        assertEquals(stateIds, fixture.narrativeStates.map { it.id })
        assertEquals(
            listOf("spaces-home", "compose", "newswire", "apps-checklists", "nearby", "offline-copy"),
            fixture.narrativeStates.map { it.surface },
        )
        assertEquals(
            listOf(
                "Your community. Your newswire.",
                "Publish signed updates from the field.",
                "Read signatures and community editorial labels.",
                "Carry useful tools with the community.",
                "Exchange updates nearby.",
                "Keep a local copy available offline.",
            ),
            fixture.narrativeStates.map { it.headline },
        )
        assertTrue(fixture.narrativeStates[4].supportingCopy.startsWith("Experimental:"))
    }

    @Test
    fun keysAndDeterministicIdentifiersAreExact() {
        val root = JSONObject(fixtureBytes().toString(Charsets.UTF_8))
        assertEquals(
            setOf("schemaVersion", "fixtureRevision", "fixtureKind", "fixedClock", "identifiers", "narrativeStates"),
            root.keys().asSequence().toSet(),
        )
        val identifiers = root.getJSONObject("identifiers")
        assertEquals(setOf("communityId", "contributorId", "stateIds"), identifiers.keys().asSequence().toSet())
        val states = root.getJSONArray("narrativeStates")
        repeat(states.length()) { index ->
            assertEquals(
                setOf("id", "surface", "headline", "supportingCopy", "communityId", "contributorId", "entryId"),
                states.getJSONObject(index).keys().asSequence().toSet(),
            )
        }

        val fixture = ReleaseFixture.decode(fixtureBytes())
        assertIdentifier(fixture.identifiers.communityId, "riot-release-fixture:v1:community")
        assertIdentifier(fixture.identifiers.contributorId, "riot-release-fixture:v1:contributor")
        stateIds.forEach { id ->
            assertIdentifier(checkNotNull(fixture.identifiers.stateIds[id]), "riot-release-fixture:v1:$id")
        }
    }

    @Test
    fun publicDigestAndSemanticMutationsHaveTypedFailures() {
        assertFailure(ReleaseFixtureContractError.DIGEST_MISMATCH) {
            ReleaseFixture.decode(fixtureBytes() + ' '.code.toByte())
        }
        assertMutation(ReleaseFixtureContractError.INVALID_ROOT_KEYS) { it.put("unknown", true) }
        assertMutation(ReleaseFixtureContractError.INVALID_IDENTIFIER_KEYS) {
            it.getJSONObject("identifiers").put("unknown", true)
        }
        assertMutation(ReleaseFixtureContractError.INVALID_STATE_KEYS) {
            it.getJSONArray("narrativeStates").getJSONObject(0).put("unknown", true)
        }
        assertMutation(ReleaseFixtureContractError.INVALID_FIXED_CLOCK) {
            it.put("fixedClock", "2026-07-24T12:00:01.000Z")
        }
        assertMutation(ReleaseFixtureContractError.NON_SYNTHETIC_KIND) { it.put("fixtureKind", "real") }
        assertMutation(ReleaseFixtureContractError.INVALID_IDENTIFIER) {
            it.getJSONObject("identifiers").put("communityId", "short")
        }
        assertMutation(ReleaseFixtureContractError.MISMATCHED_STATE_IDENTIFIER) {
            it.getJSONArray("narrativeStates").getJSONObject(0).put("entryId", "0".repeat(64))
        }
        assertMutation(ReleaseFixtureContractError.INVALID_STATE_INVENTORY) {
            it.getJSONArray("narrativeStates").remove(5)
        }
        assertMutation(ReleaseFixtureContractError.INVALID_STATE_INVENTORY) {
            val states = it.getJSONArray("narrativeStates")
            states.put(JSONObject(states.getJSONObject(0).toString()))
        }
        assertMutation(ReleaseFixtureContractError.INVALID_STATE_INVENTORY) {
            val states = it.getJSONArray("narrativeStates")
            val first = states.getJSONObject(0)
            states.put(0, states.getJSONObject(1))
            states.put(1, first)
        }
    }

    @Test
    fun everyProhibitedKeyAndValueFailsClosed() {
        listOf(
            "person", "personName", "name", "email", "phone", "location", "address",
            "latitude", "longitude", "coordinates", "notification", "deviceToken",
            "apnsToken", "fcmToken", "private", "privateCommunity", "url", "hostname",
            "ipAddress", "npub", "nsec", "note",
        ).forEach { key ->
            assertMutation(ReleaseFixtureContractError.PROHIBITED_DATA) {
                it.getJSONArray("narrativeStates").getJSONObject(0).put(key, "synthetic-test-value")
            }
        }
        listOf(
            "Person: Ana", "ana@example.com", "+64 21 555 0100", "123 Main Street",
            "37.7749,-122.4194", "ExponentPushToken[fixture]", "private community",
            "https://riot.protest.net", "riot.protest.net", "203.0.113.1",
            "npub1t985dmat80n6xlrnhsjzzrlhfkcmmemul47n3mz9lws70lrxs0pqwzdyaw",
            "nsec1tu92893lv55urd4almqhfnrv48ls2uwas5hxg6eashq9jhnt45ts9en3zd",
            "note1m99r7nwc0wdrkzldrqan96gklg5usqspq7z9696j6unf0ljnpxjspqfw99",
        ).forEach { value ->
            assertMutation(ReleaseFixtureContractError.PROHIBITED_DATA) {
                it.getJSONArray("narrativeStates").getJSONObject(0).put("supportingCopy", value)
            }
        }
    }

    private fun fixtureBytes(): ByteArray =
        checkNotNull(javaClass.classLoader!!.getResourceAsStream("riot-1.0-synthetic.json")) {
            "riot-1.0-synthetic.json missing from test classpath"
        }.use { it.readBytes() }

    private fun assertIdentifier(identifier: String, label: String) {
        assertEquals(sha256(label.toByteArray()), identifier)
        assertTrue(identifier.matches(Regex("[0-9a-f]{64}")))
    }

    private fun assertMutation(error: ReleaseFixtureContractError, mutate: (JSONObject) -> Unit) {
        val root = JSONObject(fixtureBytes().toString(Charsets.UTF_8))
        mutate(root)
        assertFailure(error) { ReleaseFixture.validateSemantics(root.toString().toByteArray()) }
    }

    private fun assertFailure(error: ReleaseFixtureContractError, action: () -> Unit) {
        try {
            action()
            fail("Expected $error")
        } catch (caught: ReleaseFixtureContractException) {
            assertEquals(error, caught.error)
        }
    }

    private fun sha256(bytes: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
}
