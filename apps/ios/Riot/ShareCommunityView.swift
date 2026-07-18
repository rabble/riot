import CoreImage
import SwiftUI

#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

// MARK: - Share a community (link + QR)

/// The one seam the share screen reads: a held newswire descriptor minted into a
/// digest-bound `riot://newswire/join/v1/...` reference. `RiotProfileRepository`
/// conforms (its `newswireShareReference` already does the work); tests inject a
/// stub so the resolver and view model are provable without a live store.
public protocol NewswireShareReferencing {
    func newswireShareReference(spaceDescriptorEntryID: String) throws -> NewswireShareReference
}

extension RiotProfileRepository: NewswireShareReferencing {}

/// What the share screen can show for a community. Either a ready-to-share
/// canonical link, or an honest reason it can't be shared yet — a community
/// whose signed descriptor has not arrived over sync carries no id to mint from,
/// and a mint failure is surfaced as a plain message, never a raw error.
public enum ShareCommunityOutcome: Equatable, Sendable {
    case ready(link: String)
    case unavailable(message: String)
}

/// Resolves a community's share link from its descriptor id. Pure over the
/// injected `NewswireShareReferencing`, so the empty-descriptor and mint-failure
/// paths are testable without CoreImage or a live profile.
public enum ShareCommunityResolver {
    /// A joined/pending community carries no descriptor id until its signed
    /// descriptor arrives over sync; it cannot be shared before then.
    public static let missingDescriptorMessage =
        "This community can't be shared yet — its details arrive the next time you sync."
    /// The descriptor is known but minting the reference failed. Fixed copy, never
    /// the raw error.
    public static let mintFailureMessage =
        "Couldn't build a share link for this community. Try again in a moment."

    public static func resolve(
        spaceDescriptorEntryID: String,
        referencing: NewswireShareReferencing
    ) -> ShareCommunityOutcome {
        let trimmed = spaceDescriptorEntryID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return .unavailable(message: missingDescriptorMessage) }
        do {
            let reference = try referencing.newswireShareReference(spaceDescriptorEntryID: trimmed)
            return .ready(link: reference.encoded)
        } catch {
            return .unavailable(message: mintFailureMessage)
        }
    }
}

/// Renders a share string into a crisp QR code. Pure CoreImage — no camera, no
/// network — so it is fully verifiable off-device. The generated image scales
/// each QR module to an integer block (nearest-neighbour), so it stays sharp
/// when the view scales it further with `.interpolation(.none)`.
public enum CommunityQRCode {
    /// A `CGImage` QR of `string`, or `nil` if CoreImage can't build one (which it
    /// never does for a non-empty ASCII share link, but the caller degrades
    /// honestly rather than force-unwrapping).
    public static func cgImage(for string: String, moduleScale: CGFloat = 12) -> CGImage? {
        guard !string.isEmpty, let filter = CIFilter(name: "CIQRCodeGenerator") else { return nil }
        filter.setValue(Data(string.utf8), forKey: "inputMessage")
        // Medium error correction — a good balance of density and scan tolerance.
        filter.setValue("M", forKey: "inputCorrectionLevel")
        guard let output = filter.outputImage else { return nil }
        let scaled = output.transformed(by: CGAffineTransform(scaleX: moduleScale, y: moduleScale))
        let context = CIContext()
        return context.createCGImage(scaled, from: scaled.extent)
    }
}

/// Holds the resolved share outcome for one community. `@MainActor` because it
/// reads the profile seam; resolution is synchronous, so the outcome is settled
/// at init and the view binds to it directly.
@MainActor
public final class ShareCommunityModel: ObservableObject {
    public let communityName: String
    @Published public private(set) var outcome: ShareCommunityOutcome

    public init(
        communityName: String,
        spaceDescriptorEntryID: String,
        referencing: NewswireShareReferencing
    ) {
        self.communityName = communityName
        self.outcome = ShareCommunityResolver.resolve(
            spaceDescriptorEntryID: spaceDescriptorEntryID,
            referencing: referencing
        )
    }

    /// The canonical link when one is ready, else `nil` — what Copy and Share act on.
    public var shareLink: String? {
        if case let .ready(link) = outcome { return link }
        return nil
    }
}

// MARK: - View

/// Share-a-community screen: shows the canonical join link as a scannable QR
/// code, the link text, a Copy button, and a system share sheet. Someone on the
/// same table scans the QR; someone remote gets the link. Themed like the rest
/// of the app (paper/ink, riotHeader, RiotCard). A community whose descriptor
/// has not synced yet shows an honest "can't share yet" state rather than a
/// broken code.
public struct ShareCommunityView: View {
    @StateObject private var model: ShareCommunityModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.dismiss) private var dismiss
    @State private var didCopy = false

    public init(
        communityName: String,
        spaceDescriptorEntryID: String,
        referencing: NewswireShareReferencing
    ) {
        _model = StateObject(wrappedValue: ShareCommunityModel(
            communityName: communityName,
            spaceDescriptorEntryID: spaceDescriptorEntryID,
            referencing: referencing
        ))
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    switch model.outcome {
                    case let .ready(link):
                        shareCard(link: link)
                    case let .unavailable(message):
                        unavailableCard(message)
                    }
                    // An explicit Done inside the content, because `riotHeader`
                    // hides the iOS navigation bar (so a toolbar button wouldn't
                    // render) and a macOS sheet needs a deliberate dismiss.
                    Button("Done") { dismiss() }
                        .buttonStyle(.riotSecondary)
                        .accessibilityIdentifier("share-community-done")
                }
                .padding(20)
            }
            .background(RiotTheme.paper(for: colorScheme))
            .riotHeader(eyebrow: "Invite people to join", "Share a community")
        }
    }

    private func shareCard(link: String) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 16) {
                eyebrow(model.communityName)
                qrCode(for: link)
                Text(link)
                    .font(.riot(.mono, size: 13, relativeTo: .footnote))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    .textSelection(.enabled)
                    .accessibilityIdentifier("share-community-link")
                HStack(spacing: 12) {
                    Button(didCopy ? "Copied" : "Copy link") { copy(link) }
                        .buttonStyle(.riotSecondary)
                        .accessibilityIdentifier("share-community-copy")
                    ShareLink(item: link) {
                        Text("Share")
                            .font(.riot(.mono, size: 13, relativeTo: .footnote))
                            .textCase(.uppercase)
                            .tracking(1)
                            .padding(.horizontal, 22)
                            .padding(.vertical, 14)
                            .foregroundStyle(RiotTheme.paper(for: colorScheme))
                            .background(RiotTheme.ink(for: colorScheme))
                            .overlay(Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2))
                    }
                    .accessibilityIdentifier("share-community-share")
                }
            }
        }
    }

    @ViewBuilder
    private func qrCode(for link: String) -> some View {
        if let cgImage = CommunityQRCode.cgImage(for: link) {
            Image(decorative: cgImage, scale: 1, orientation: .up)
                .interpolation(.none)
                .resizable()
                .aspectRatio(1, contentMode: .fit)
                .frame(maxWidth: 240)
                .padding(12)
                .background(Color.white)
                .overlay(Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2))
                .frame(maxWidth: .infinity)
                .accessibilityIdentifier("share-community-qr")
                .accessibilityLabel("QR code to join \(model.communityName)")
        }
    }

    private func unavailableCard(_ message: String) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                eyebrow(model.communityName)
                Text(message)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityIdentifier("share-community-unavailable")
            }
        }
    }

    private func eyebrow(_ text: String) -> some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.pink(for: colorScheme))
    }

    private func copy(_ link: String) {
        #if canImport(UIKit)
        UIPasteboard.general.string = link
        #elseif canImport(AppKit)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(link, forType: .string)
        #endif
        didCopy = true
    }
}
