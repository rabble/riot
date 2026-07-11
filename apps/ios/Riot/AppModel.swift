import Foundation
import SwiftUI

public enum RiotDestination: String, CaseIterable, Identifiable, Sendable {
    case spaces
    case board
    case compose
    case importPreview
    case connection

    public var id: Self { self }

    public static let phoneTabs = allCases

    public var title: String {
        switch self {
        case .spaces: "Spaces"
        case .board: "Incident board"
        case .compose: "Compose & sign"
        case .importPreview: "Import preview"
        case .connection: "Connection"
        }
    }

    public var tabTitle: String {
        switch self {
        case .spaces: "Spaces"
        case .board: "Board"
        case .compose: "Compose"
        case .importPreview: "Import"
        case .connection: "Connect"
        }
    }

    public var systemImage: String {
        switch self {
        case .spaces: "square.stack.3d.up"
        case .board: "exclamationmark.bubble"
        case .compose: "square.and.pencil"
        case .importPreview: "tray.and.arrow.down"
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
    @Published public private(set) var importEntries: [RiotEntry] = []
    @Published public private(set) var connectionStatus: RiotConnectionStatus = .offline
    @Published public private(set) var errorMessage: String?

    private var repository: RiotProfileRepository?

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

    public func bootstrap() {
        guard repository == nil else { return }
        do {
            let base = try FileManager.default.url(
                for: .applicationSupportDirectory,
                in: .userDomainMask,
                appropriateFor: nil,
                create: true
            )
            let storage = try ProtectedProfileStorage(fileURL: base.appendingPathComponent("riot-profile.json"))
            let repository = try RiotProfileRepository.open(storage: storage)
            self.repository = repository
            space = repository.currentSpace
            entries = try repository.currentEntries()
        } catch {
            errorMessage = String(describing: error)
        }
    }

    public func createSpace(title: String) {
        perform {
            guard let repository else { return }
            space = try repository.createPublicSpace(title: title)
            destination = .board
        }
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
}
