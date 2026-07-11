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

fun PersistedSpace.toPublicSpace() = PublicSpace(
    namespaceId = namespaceId,
    title = title,
    isPublic = true,
)
