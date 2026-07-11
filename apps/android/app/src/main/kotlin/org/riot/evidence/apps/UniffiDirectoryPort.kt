package org.riot.evidence.apps

import uniffi.riot_ffi.AppRuntimeSession
import uniffi.riot_ffi.DirectoryListing
import uniffi.riot_ffi.PublicSpace

/** [DirectoryPort] backed by the real Rust app-runtime session. */
class UniffiDirectoryPort(private val session: AppRuntimeSession) : DirectoryPort {
    override fun listings(): List<DirectoryListing> = session.directoryListings()

    override fun endorse(appId: ByteArray, note: String, retract: Boolean) =
        session.endorseApp(appId, note, retract)

    override fun share(appId: ByteArray, space: PublicSpace) =
        session.shareApp(appId, space)
}
