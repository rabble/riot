import SwiftUI

/// Show the current community's join reference so others can follow it — a
/// canonical `riot://newswire/join/v1/...` link (via the system share sheet) and,
/// on iOS, a locally rendered QR of that same link. The link is minted by core
/// through ``ShareCommunityModel`` from the community's descriptor entry id; when
/// that id isn't known on this device yet (a joined community before its first
/// sync) the sheet says so honestly instead of fabricating a link. macOS shows
/// the link only — no camera-facing QR is needed there, and `ShareLink` still
/// hands the link to the system share menu.
///
/// Presented from ``CommunitySettingsSheet``. Nothing here touches the network or
/// invents content: the QR is pure CoreImage over the exact string core produced.
public struct ShareCommunitySheet: View {
    private let community: CommunityContext
    private let resolveEncoded: (String) throws -> String
    private let onClose: () -> Void

    private let model = ShareCommunityModel()

    public init(
        community: CommunityContext,
        resolveEncoded: @escaping (String) throws -> String,
        onClose: @escaping () -> Void
    ) {
        self.community = community
        self.resolveEncoded = resolveEncoded
        self.onClose = onClose
    }

    /// The share decision, computed from the injected community + resolver. Exposed
    /// (internal) so the wiring is unit-testable without driving the view: a held
    /// descriptor is shareable, a nil/failed/foreign one is `.unavailable`.
    var content: ShareCommunityContent {
        model.content(
            descriptorEntryID: community.newswireDescriptorEntryID,
            resolveEncoded: resolveEncoded
        )
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    switch content {
                    case let .shareable(link):
                        shareableCard(link)
                    case .unavailable:
                        unavailableCard
                    }
                }
                .padding(20)
            }
            .riotHeader(eyebrow: "Share", "Share this community")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done", action: onClose)
                        .accessibilityIdentifier("share-community-done")
                }
            }
        }
    }

    private func shareableCard(_ link: String) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text("Anyone with this link or QR code can follow \(community.name).")
                    .font(.riot(.body, size: 14, relativeTo: .body))
                    .foregroundStyle(.secondary)

                #if os(iOS)
                if let qr = QRImageRenderer.makeQRCode(from: link) {
                    Image(decorative: qr, scale: 1)
                        .interpolation(.none)
                        .resizable()
                        .aspectRatio(1, contentMode: .fit)
                        .frame(maxWidth: 220)
                        .accessibilityIdentifier("share-community-qr")
                        .accessibilityLabel("QR code to follow \(community.name)")
                }
                #endif

                Text(link)
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("share-community-link")

                ShareLink(item: link) {
                    Text("Share link")
                }
                .buttonStyle(.riotPrimary)
                .accessibilityIdentifier("share-community-sharelink")
            }
        }
        .accessibilityIdentifier("share-community-shareable")
    }

    private var unavailableCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 8) {
                Text("Nothing to share yet")
                    .font(.riot(.monoBold, size: 17, relativeTo: .headline))
                Text("This community's link becomes available once its details reach this device on first sync. Check back in a moment.")
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityIdentifier("share-community-unavailable")
    }
}
