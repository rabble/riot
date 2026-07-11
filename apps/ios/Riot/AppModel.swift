import Foundation
import SwiftUI

public enum RiotDestination: String, CaseIterable, Identifiable, Sendable {
    case spaces
    case directory
    case board
    case compose
    case connection

    public var id: Self { self }

    public static let phoneTabs = allCases

    public var title: String {
        switch self {
        case .spaces: "Spaces"
        case .directory: "App directory"
        case .board: "Incident board"
        case .compose: "Compose & sign"
        case .connection: "Connection"
        }
    }

    public var tabTitle: String {
        switch self {
        case .spaces: "Spaces"
        case .directory: "Apps"
        case .board: "Board"
        case .compose: "Compose"
        case .connection: "Connect"
        }
    }

    public var systemImage: String {
        switch self {
        case .spaces: "square.stack.3d.up"
        case .directory: "square.grid.2x2"
        case .board: "exclamationmark.bubble"
        case .compose: "square.and.pencil"
        case .connection: "antenna.radiowaves.left.and.right"
        }
    }
}

public enum RiotConnectionStatus: Equatable, Sendable {
    case offline
    case nearby(String)
}

@MainActor
public final class RiotAppModel: ObservableObject {
    @Published public var destination: RiotDestination = .spaces
    @Published public private(set) var space: RiotSpace?
    @Published public private(set) var entries: [RiotEntry] = []
    @Published public private(set) var apps: [RiotSpaceApp] = []
    @Published public private(set) var connectionStatus: RiotConnectionStatus = .offline
    @Published public private(set) var errorMessage: String?

    private var repository: RiotProfileRepository?

    /// Read-only handle for the runtime host, which needs the live repository to
    /// mount a trusted app's WebView. Exposed instead of widening the stored
    /// property so callers cannot swap the repository out from under the model.
    public var profileRepository: RiotProfileRepository? { repository }

    public init() {}

    init(testError: String) {
        errorMessage = testError
    }

    public var connectionDisclosure: String {
        switch connectionStatus {
        case .offline: "Offline · local device only"
        case let .nearby(peer): "Nearby · \(peer)"
        }
    }

    public func select(_ destination: RiotDestination) {
        self.destination = destination
    }

    public func dismissError() {
        errorMessage = nil
    }

    public func openNearbySyncBoundary() throws -> MobileSyncSessionBoundary {
        guard let repository else { throw RepositoryError.profileClosed }
        return try repository.openSyncBoundary()
    }

    /// Opens (or restores) the on-device profile and installs the starter tools.
    ///
    /// `storageDirectory`, `keyStore`, and `starterPacks` all carry their
    /// production defaults; tests override them to isolate storage, skip the
    /// Keychain, and force the starter set (an empty list emulates the packs
    /// failing to load). When `starterPacks` is nil the packs are read from the
    /// app bundle, falling back to the source tree on a DEBUG simulator run.
    public func bootstrap(
        storageDirectory: URL? = nil,
        keyStore: WrappingKeyStore = KeychainWrappingKeyStore(),
        starterPacks: [(manifest: Data, bundle: Data)]? = nil
    ) {
        guard repository == nil else { return }
        do {
            let base = try storageDirectory ?? FileManager.default.url(
                for: .applicationSupportDirectory,
                in: .userDomainMask,
                appropriateFor: nil,
                create: true
            )
            let storage = try ProtectedProfileStorage(fileURL: base.appendingPathComponent("riot-profile.json"))
            let repository = try RiotProfileRepository.open(
                storage: storage,
                keyStore: keyStore,
                starterPacks: starterPacks ?? Self.loadStarterPacks()
            )
            self.repository = repository
            space = repository.currentSpace
            entries = try repository.currentEntries()
            refreshApps()
        } catch {
            errorMessage = String(describing: error)
        }
    }

    public func createSpace(title: String) {
        perform {
            guard let repository else { return }
            space = try repository.createPublicSpace(title: title)
            refreshApps()
            destination = .board
        }
    }

    /// Trusts an app in this space so everyone here can use it, then refreshes
    /// the listing so the row flips from "Review" to "Open".
    public func trustApp(appID: String) {
        perform {
            try repository?.trustApp(appID: appID)
            refreshApps()
        }
    }

    private func refreshApps() {
        apps = (try? repository?.spaceApps()) ?? []
    }

    public func sign(headline: String, description: String, aiAssisted: Bool) {
        perform {
            guard let repository, let space else { return }
            let expiry = UInt64(Date().timeIntervalSince1970) + 3_600
            _ = try repository.signAlert(
                in: space,
                draft: AlertDraft(
                    expiresAt: expiry,
                    headline: headline,
                    description: description,
                    sourceClaims: ["Local conference participant"],
                    aiAssisted: aiAssisted
                )
            )
            entries = try repository.currentEntries()
            destination = .board
        }
    }

    private func perform(_ operation: () throws -> Void) {
        do {
            try operation()
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    // MARK: - Starter packs

    /// The frozen starter catalog to install on open. A pair that cannot be read
    /// is dropped (Rust remains the integrity oracle for the bytes we do read),
    /// so a missing artifact leaves the Tools list empty rather than failing
    /// `bootstrap`.
    private static func loadStarterPacks() -> [(manifest: Data, bundle: Data)] {
        [("checklist.manifest", "checklist.bundle")].compactMap { manifestName, bundleName in
            guard let manifest = loadPackData(named: manifestName),
                  let bundle = loadPackData(named: bundleName)
            else { return nil }
            return (manifest: manifest, bundle: bundle)
        }
    }

    private static func loadPackData(named name: String) -> Data? {
        for url in packURLs(named: name) {
            if let data = try? Data(contentsOf: url) { return data }
        }
        return nil
    }

    /// Candidate locations for a `.cbor` starter artifact, in order: the app
    /// bundle (device/release), then the checked-in fixtures resolved from this
    /// source file (DEBUG only — the path exists on the host filesystem a
    /// simulator shares, but not on a device).
    private static func packURLs(named name: String) -> [URL] {
        var urls: [URL] = []
        if let bundled = Bundle.main.url(forResource: name, withExtension: "cbor") {
            urls.append(bundled)
        }
        #if DEBUG
        urls.append(sourceTreeFixtures().appendingPathComponent("\(name).cbor"))
        #endif
        return urls
    }

    /// `fixtures/apps` resolved four levels up from this file at
    /// `apps/ios/Riot/AppModel.swift`, matching the repository tests' convention.
    private static func sourceTreeFixtures(file: StaticString = #filePath) -> URL {
        URL(fileURLWithPath: "\(file)")
            .deletingLastPathComponent() // Riot
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
            .appendingPathComponent("fixtures/apps")
    }
}
