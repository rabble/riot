package org.riot.evidence

import java.security.MessageDigest
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

enum class ReleaseFixtureContractError {
    DIGEST_MISMATCH,
    MALFORMED_JSON,
    INVALID_ROOT_KEYS,
    INVALID_IDENTIFIER_KEYS,
    INVALID_STATE_KEYS,
    INVALID_SCHEMA_VERSION,
    INVALID_FIXTURE_REVISION,
    NON_SYNTHETIC_KIND,
    INVALID_FIXED_CLOCK,
    INVALID_STATE_INVENTORY,
    INVALID_IDENTIFIER,
    MISMATCHED_STATE_IDENTIFIER,
    INVALID_NARRATIVE_VALUE,
    PROHIBITED_DATA,
}

class ReleaseFixtureContractException(
    val error: ReleaseFixtureContractError,
) : IllegalArgumentException(error.name)

data class ReleaseFixtureIdentifiers(
    val communityId: String,
    val contributorId: String,
    val stateIds: Map<String, String>,
)

data class ReleaseFixtureNarrativeState(
    val id: String,
    val surface: String,
    val headline: String,
    val supportingCopy: String,
    val communityId: String,
    val contributorId: String,
    val entryId: String,
)

data class ReleaseFixture(
    val schemaVersion: Int,
    val fixtureRevision: String,
    val fixtureKind: String,
    val fixedClock: String,
    val identifiers: ReleaseFixtureIdentifiers,
    val narrativeStates: List<ReleaseFixtureNarrativeState>,
) {
    companion object {
        private const val PINNED_DIGEST =
            "930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469"
        private val ROOT_KEYS = setOf(
            "schemaVersion", "fixtureRevision", "fixtureKind", "fixedClock",
            "identifiers", "narrativeStates",
        )
        private val IDENTIFIER_KEYS = setOf("communityId", "contributorId", "stateIds")
        private val STATE_KEYS = setOf(
            "id", "surface", "headline", "supportingCopy",
            "communityId", "contributorId", "entryId",
        )
        private val EXPECTED_STATES = listOf(
            ExpectedState(
                "community-newswire",
                "spaces-home",
                "Your community. Your newswire.",
                "Follow updates from a community you choose.",
            ),
            ExpectedState(
                "signed-publishing",
                "compose",
                "Publish signed updates from the field.",
                "Signatures show source and integrity, not whether a claim is true.",
            ),
            ExpectedState(
                "labels-and-signatures",
                "newswire",
                "Read signatures and community editorial labels.",
                "See signed source details. Community editorial labels are community signals, not independent factual verification.",
            ),
            ExpectedState(
                "community-tools",
                "apps-checklists",
                "Carry useful tools with the community.",
                "Open a shared checklist alongside community updates.",
            ),
            ExpectedState(
                "nearby-exchange",
                "nearby",
                "Exchange updates nearby.",
                "Experimental: exchange updates directly with a nearby device.",
            ),
            ExpectedState(
                "offline-copy",
                "offline-copy",
                "Keep a local copy available offline.",
                "Keep a local copy ready to read without a connection.",
            ),
        )
        private val PROHIBITED_KEYS = setOf(
            "person", "personname", "name", "email", "phone", "location", "address",
            "latitude", "longitude", "coordinates", "notification", "devicetoken",
            "apnstoken", "fcmtoken", "private", "privatecommunity", "url", "hostname",
            "ipaddress", "npub", "nsec", "note",
        )

        fun decode(bytes: ByteArray): ReleaseFixture {
            requireContract(sha256(bytes) == PINNED_DIGEST, ReleaseFixtureContractError.DIGEST_MISMATCH)
            return validateSemantics(bytes)
        }

        internal fun validateSemantics(bytes: ByteArray): ReleaseFixture {
            val tokener = JSONTokener(bytes.toString(Charsets.UTF_8))
            val parsed = try {
                tokener.nextValue()
            } catch (_: Exception) {
                fail(ReleaseFixtureContractError.MALFORMED_JSON)
            }
            if (parsed !is JSONObject) {
                fail(ReleaseFixtureContractError.MALFORMED_JSON)
            }
            // JSONTokener stops after the root object; only trailing whitespace
            // (for example the final LF) may follow, anything else is malformed.
            while (tokener.more()) {
                if (!tokener.next().isWhitespace()) {
                    fail(ReleaseFixtureContractError.MALFORMED_JSON)
                }
            }
            val root = parsed
            rejectProhibitedData(root)
            requireContract(root.keysSet() == ROOT_KEYS, ReleaseFixtureContractError.INVALID_ROOT_KEYS)

            val identifiersObject = root.optJSONObject("identifiers")
                ?: fail(ReleaseFixtureContractError.INVALID_IDENTIFIER_KEYS)
            requireContract(
                identifiersObject.keysSet() == IDENTIFIER_KEYS,
                ReleaseFixtureContractError.INVALID_IDENTIFIER_KEYS,
            )
            val statesArray = try {
                root.getJSONArray("narrativeStates")
            } catch (_: Exception) {
                fail(ReleaseFixtureContractError.MALFORMED_JSON)
            }
            repeat(statesArray.length()) { index ->
                val state = try {
                    statesArray.getJSONObject(index)
                } catch (_: Exception) {
                    fail(ReleaseFixtureContractError.MALFORMED_JSON)
                }
                requireContract(state.keysSet() == STATE_KEYS, ReleaseFixtureContractError.INVALID_STATE_KEYS)
            }

            val fixture = try {
                decodeObject(root, identifiersObject, statesArray)
            } catch (error: ReleaseFixtureContractException) {
                throw error
            } catch (_: Exception) {
                fail(ReleaseFixtureContractError.MALFORMED_JSON)
            }
            fixture.validate()
            return fixture
        }

        private fun decodeObject(
            root: JSONObject,
            identifiersObject: JSONObject,
            statesArray: JSONArray,
        ): ReleaseFixture {
            val stateIdsObject = identifiersObject.getJSONObject("stateIds")
            val stateIds = stateIdsObject.keys().asSequence().associateWith { stateIdsObject.getString(it) }
            val states = (0 until statesArray.length()).map { index ->
                statesArray.getJSONObject(index).run {
                    ReleaseFixtureNarrativeState(
                        id = getString("id"),
                        surface = getString("surface"),
                        headline = getString("headline"),
                        supportingCopy = getString("supportingCopy"),
                        communityId = getString("communityId"),
                        contributorId = getString("contributorId"),
                        entryId = getString("entryId"),
                    )
                }
            }
            return ReleaseFixture(
                schemaVersion = exactSchemaVersion(root),
                fixtureRevision = root.getString("fixtureRevision"),
                fixtureKind = root.getString("fixtureKind"),
                fixedClock = root.getString("fixedClock"),
                identifiers = ReleaseFixtureIdentifiers(
                    communityId = identifiersObject.getString("communityId"),
                    contributorId = identifiersObject.getString("contributorId"),
                    stateIds = stateIds,
                ),
                narrativeStates = states,
            )
        }

        // org.json getInt truncates 1.9 to 1; Swift JSONDecoder rejects any
        // non-integral or out-of-range schemaVersion as malformed. Match that
        // exactly: only an integral number representable as Int is accepted.
        private fun exactSchemaVersion(root: JSONObject): Int = when (val raw = root.get("schemaVersion")) {
            is Int -> raw
            is Long -> if (raw.toInt().toLong() == raw) raw.toInt() else {
                fail(ReleaseFixtureContractError.MALFORMED_JSON)
            }
            is Double -> if (raw % 1.0 == 0.0 && raw.toInt().toDouble() == raw) raw.toInt() else {
                fail(ReleaseFixtureContractError.MALFORMED_JSON)
            }
            else -> fail(ReleaseFixtureContractError.MALFORMED_JSON)
        }

        private fun ReleaseFixture.validate() {
            requireContract(schemaVersion == 1, ReleaseFixtureContractError.INVALID_SCHEMA_VERSION)
            requireContract(
                fixtureRevision == "riot-1.0-synthetic-v1",
                ReleaseFixtureContractError.INVALID_FIXTURE_REVISION,
            )
            requireContract(fixtureKind == "synthetic", ReleaseFixtureContractError.NON_SYNTHETIC_KIND)
            requireContract(
                fixedClock == "2026-07-24T12:00:00.000Z",
                ReleaseFixtureContractError.INVALID_FIXED_CLOCK,
            )
            val expectedIds = EXPECTED_STATES.map { it.id }
            requireContract(
                narrativeStates.map { it.id } == expectedIds &&
                    identifiers.stateIds.keys == expectedIds.toSet(),
                ReleaseFixtureContractError.INVALID_STATE_INVENTORY,
            )

            requireIdentifier(identifiers.communityId, "riot-release-fixture:v1:community")
            requireIdentifier(identifiers.contributorId, "riot-release-fixture:v1:contributor")
            EXPECTED_STATES.forEachIndexed { index, expected ->
                val stateIdentifier = identifiers.stateIds[expected.id]
                    ?: fail(ReleaseFixtureContractError.INVALID_STATE_INVENTORY)
                requireIdentifier(stateIdentifier, "riot-release-fixture:v1:${expected.id}")
                val state = narrativeStates[index]
                requireContract(
                    state.surface == expected.surface &&
                        state.headline == expected.headline &&
                        state.supportingCopy == expected.copy,
                    ReleaseFixtureContractError.INVALID_NARRATIVE_VALUE,
                )
                requireContract(
                    state.communityId == identifiers.communityId &&
                        state.contributorId == identifiers.contributorId &&
                        state.entryId == stateIdentifier,
                    ReleaseFixtureContractError.MISMATCHED_STATE_IDENTIFIER,
                )
            }
        }

        private fun requireIdentifier(identifier: String, label: String) {
            requireContract(
                identifier.matches(Regex("[0-9a-f]{64}")) &&
                    identifier == sha256(label.toByteArray()),
                ReleaseFixtureContractError.INVALID_IDENTIFIER,
            )
        }

        private fun rejectProhibitedData(value: Any?) {
            when (value) {
                is JSONObject -> value.keys().asSequence().forEach { key ->
                    requireContract(
                        key.lowercase() !in PROHIBITED_KEYS,
                        ReleaseFixtureContractError.PROHIBITED_DATA,
                    )
                    rejectProhibitedData(value.get(key))
                }
                is JSONArray -> repeat(value.length()) { rejectProhibitedData(value.get(it)) }
                is String -> requireContract(
                    !isProhibitedValue(value),
                    ReleaseFixtureContractError.PROHIBITED_DATA,
                )
            }
        }

        private fun isProhibitedValue(value: String): Boolean {
            val lower = value.lowercase()
            return listOf(
                "person: ana", "ana@example.com", "+64 21 555 0100", "123 main street",
                "37.7749,-122.4194", "exponentpushtoken[fixture]", "private community",
                "https://", "http://", "riot.protest.net", "203.0.113.1",
            ).any(lower::contains) ||
                lower.startsWith("npub1") ||
                lower.startsWith("nsec1") ||
                lower.startsWith("note1")
        }

        private fun JSONObject.keysSet(): Set<String> = keys().asSequence().toSet()

        private fun sha256(bytes: ByteArray): String =
            MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

        private fun requireContract(condition: Boolean, error: ReleaseFixtureContractError) {
            if (!condition) fail(error)
        }

        private fun fail(error: ReleaseFixtureContractError): Nothing =
            throw ReleaseFixtureContractException(error)
    }
}

private data class ExpectedState(
    val id: String,
    val surface: String,
    val headline: String,
    val copy: String,
)
