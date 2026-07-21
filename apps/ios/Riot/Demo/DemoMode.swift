import SwiftUI

/// Demo mode: the seeded *Riverside Tenants Union* space, loaded on demand from
/// a real signed evidence bundle shipped in the app's resources.
///
/// Nothing here is a shortcut. The bundle goes through the ordinary
/// `inspect → plan → commit` import — the same one a bundle from a phone across
/// the room goes through — so what the demo shows is the product, not a picture
/// of it. If the import pipeline broke, the demo would break with it, which is
/// the point.
///
/// The toggle is deliberately hard to reach: a long press on the version string.
/// It is not a feature, it is a stage prop, and nobody should find it by
/// wandering.

// MARK: - The fixture

/// The seeded bundle as it ships: a resource in the app bundle, copied there by
/// the app target's Resources build phase.
///
/// **There is no `#if DEBUG` fallback here, on purpose.** The Riot app target's
/// Debug configuration does not define `DEBUG`, so a `#if DEBUG` source-tree
/// fallback compiles away to nothing and the file is simply missing at runtime —
/// a failure that looks exactly like a missing resource and costs an afternoon.
/// The resource phase is the single way this file arrives, and `DemoModeTests`
/// is what proves it did.
public enum DemoFixture {
    public static let resourceName = "demo-space"
    public static let resourceExtension = "riot-evidence"

    /// The committed bundle's bytes, or `nil` if the resource is not in the
    /// bundle — which means the Resources build phase is missing it.
    public static func bytes(in bundle: Bundle = .main) -> Data? {
        guard
            let url = bundle.url(forResource: resourceName, withExtension: resourceExtension),
            let data = try? Data(contentsOf: url)
        else {
            return nil
        }
        return data
    }
}

// MARK: - The port

/// What demo mode needs from the profile, and nothing else.
///
/// `DemoProfileLoader` satisfies it straight from Rust, so the tests exercise the
/// real import with no test double in the way. The app's repository conforms to
/// it at integration time, where the listed space also has to be written to the
/// profile snapshot so it survives a relaunch.
public protocol DemoSpaceLoading: AnyObject {
    /// Imports the seeded bundle and lists its space. Additive: it refuses rather
    /// than displace a space the person already has.
    func loadDemoSpace(bytes: Data) throws -> RiotSpace
    /// Stops listing the demo space.
    ///
    /// Hiding is not deleting, and cannot be: Willow is append-only and there is
    /// no delete primitive. The entries stay in the local store, inert, with no
    /// space listing their namespace and nothing in the UI able to reach them.
    /// Getting the bytes back means resetting the profile.
    func hideDemoSpace() throws
}

/// The port, straight onto a profile. Deliberately an adapter rather than a
/// conformance on `MobileProfile` itself: the generated `loadDemoSpace` returns
/// the FFI's `PublicSpace`, and a same-name conformance returning `RiotSpace`
/// would overload on return type alone — legal Swift, and a trap waiting for the
/// first person who edits it.
public final class DemoProfileLoader: DemoSpaceLoading {
    private let profile: MobileProfile

    public init(profile: MobileProfile) {
        self.profile = profile
    }

    public func loadDemoSpace(bytes: Data) throws -> RiotSpace {
        let space = try profile.loadDemoSpace(bytes: bytes)
        return RiotSpace(namespaceID: space.namespaceId, title: space.title)
    }

    public func hideDemoSpace() throws {
        try profile.hideDemoSpace()
    }
}

// MARK: - The view

/// The one sentence a failure is ever allowed to say.
///
/// No codes, no "bundle", no "namespace", no "signature". A person holding a
/// phone in front of a room cannot act on any of that, and the import is
/// transactional anyway: whatever went wrong, the app is exactly where it was.
public enum DemoModeCopy {
    public static let title = "Demo mode"
    public static let load = "Load demo space"
    public static let hide = "Hide demo space"
    public static let loadFailed = "Couldn't load the demo space"
    public static let missingFixture = "Couldn't load the demo space"
    /// A real community space already exists. The demo is refused rather than
    /// displacing real data — load it on a fresh profile, or hide the demo first
    /// if you're trying to reload it.
    public static let refusedWhileSpaceExists =
        "The demo can't load alongside a real space. Hide the demo first, or reset your profile."
    /// A sync or in-flight import is open; the demo would clobber it.
    public static let refusedWhileSyncActive =
        "Finish or cancel the in-flight sync before loading the demo."
    /// Said plainly, because it is true and it is surprising.
    public static let hideExplanation =
        "Hiding stops listing the demo space. It doesn't erase it — reset the profile for that."

    /// Maps an underlying load error to the plain-language reason the person can
    /// act on, falling back to the generic message for anything unexpected.
    public static func reason(for error: Error) -> String {
        switch error as? MobileError {
        case .ImportRejected:
            return refusedWhileSpaceExists
        case .InvalidInput:
            return refusedWhileSyncActive
        default:
            return loadFailed
        }
    }
}

@MainActor
public struct DemoModeView: View {
    private let loader: DemoSpaceLoading
    private let bundle: Bundle

    @State private var loadedSpace: RiotSpace?
    @State private var failure: String?

    public init(loader: DemoSpaceLoading, bundle: Bundle = .main) {
        self.loader = loader
        self.bundle = bundle
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(DemoModeCopy.title)
                .font(.headline)

            if let loadedSpace {
                Text(loadedSpace.title)
                    .font(.subheadline)
                    .accessibilityIdentifier("demo-loaded-space")
            }

            if let failure {
                Text(failure)
                    .font(.subheadline)
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("demo-failure")
            }

            Button(DemoModeCopy.load) { load() }
                .accessibilityIdentifier("demo-load")

            Button(DemoModeCopy.hide) { hide() }
                .accessibilityIdentifier("demo-hide")

            Text(DemoModeCopy.hideExplanation)
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
        .padding()
    }

    private func load() {
        failure = nil
        guard let bytes = DemoFixture.bytes(in: bundle) else {
            // The resource is not in the bundle. The person is told the same
            // sentence as for any other failure — the distinction matters to us,
            // not to them.
            failure = DemoModeCopy.missingFixture
            return
        }
        do {
            loadedSpace = try loader.loadDemoSpace(bytes: bytes)
        } catch {
            loadedSpace = nil
            failure = DemoModeCopy.reason(for: error)
        }
    }

    private func hide() {
        failure = nil
        try? loader.hideDemoSpace()
        loadedSpace = nil
    }
}

// MARK: - The hidden toggle

/// The version string, which opens demo mode on a long press and does nothing at
/// all on a tap.
///
/// Wired into the shell in the integration pass; standalone here so it can be
/// built and reasoned about on its own.
@MainActor
public struct DemoModeVersionLabel: View {
    private let version: String
    private let loader: DemoSpaceLoading
    private let bundle: Bundle

    @State private var isPresented = false

    public init(version: String, loader: DemoSpaceLoading, bundle: Bundle = .main) {
        self.version = version
        self.loader = loader
        self.bundle = bundle
    }

    public var body: some View {
        Text(version)
            .font(.footnote)
            .foregroundStyle(.secondary)
            .accessibilityIdentifier("version-string")
            .onLongPressGesture(minimumDuration: 1.5) {
                isPresented = true
            }
            .sheet(isPresented: $isPresented) {
                DemoModeView(loader: loader, bundle: bundle)
            }
    }
}
