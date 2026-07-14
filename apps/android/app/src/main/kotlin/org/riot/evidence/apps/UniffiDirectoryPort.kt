package org.riot.evidence.apps

import uniffi.riot_ffi.AppRuntimeSession
import uniffi.riot_ffi.DirectoryListing
import uniffi.riot_ffi.PublicSpace

/**
 * [DirectoryPort] backed by the real Rust app-runtime session.
 *
 * `subspaceId` is a lambda rather than a held value because joining a space
 * REGENERATES this profile's author — a captured id would go stale the moment
 * someone joins, and the take-back affordance would then look at the wrong
 * person's endorsements. It is read fresh each time it is asked for.
 */
class UniffiDirectoryPort(
    private val session: AppRuntimeSession,
    private val subspaceId: () -> ByteArray,
) : DirectoryPort {
    override fun listings(): List<DirectoryListing> = session.directoryListings()

    override fun endorse(appId: ByteArray, note: String, retract: Boolean) =
        session.endorseApp(appId, note, retract)

    override fun share(appId: ByteArray, space: PublicSpace) =
        session.shareApp(appId, space)

    override fun mySubspaceId(): ByteArray = subspaceId()
}
