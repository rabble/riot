package org.riot.evidence

import uniffi.riot_ffi.CurrentEntry
import uniffi.riot_ffi.PublicSpace

fun CurrentEntry.toPersisted(bundleBytes: ByteArray) = PersistedAlert(
    entryId = entryId,
    namespaceId = namespaceId,
    signerId = signerId,
    headline = headline,
    createdAt = freshness.createdAt.toLong(),
    validFrom = freshness.validFrom?.toLong(),
    expiresAt = freshness.expiresAt.toLong(),
    aiAssisted = aiAssisted,
    bundleBytes = bundleBytes,
)

internal fun mergeAcceptedSync(
    profile: PersistedProfile,
    bundleBytes: ByteArray,
    entries: List<CurrentEntry>,
): PersistedProfile {
    val existingIds = profile.alerts.mapTo(mutableSetOf()) { it.entryId }
    val additions = entries
        .filter { existingIds.add(it.entryId) }
        .map { it.toPersisted(bundleBytes.copyOf()) }
    return profile.copy(alerts = profile.alerts + additions)
}

fun PersistedSpace.toPublicSpace() = PublicSpace(
    namespaceId = namespaceId,
    title = title,
    isPublic = true,
)
