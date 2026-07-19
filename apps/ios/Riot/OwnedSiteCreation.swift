import SwiftUI

// MARK: - §9.3 mandatory seizure disclosure (WU-006 Task 8a)

/// Creating an owned site mints an owned masthead namespace: the owner
/// capability lives on this phone from that moment on. §9.3 requires that,
/// before that mint happens, the person is shown a BLOCKING disclosure that
/// says plainly what device seizure means for this site — not "you might lose
/// a key," but "a captor who takes this phone unlocked can impersonate the
/// site under your name and revoke the real editors." An activist deciding
/// whether to mint a site needs that sentence, not a euphemism for it.
public enum SiteSeizureDisclosure {
    public static let title = "If this phone is seized, this site can be taken over"

    /// Deliberately does not stop at "you could lose the key" — it names the
    /// two concrete powers a seizer gains: impersonating the site, and
    /// revoking the real editors. Losing a key is recoverable with a backup;
    /// this is not a backup problem, it is a takeover risk with no undo.
    public static let body = """
        Creating this site puts its owner key on this phone. Anyone who \
        seizes this phone while it's unlocked — a captor, police, a border \
        agent — doesn't just cost you a key. They can impersonate this site \
        under your name and revoke the real editors, take over the site, and \
        lock you and your team out of it. There is no undo once that \
        happens. Only mint this site if you have a plan for keeping this \
        phone out of the wrong hands.
        """

    public static let acknowledgementLabel =
        "I understand a phone seizure can hand a captor control of this site"
}

/// Pure gate: the mint action stays disabled until the disclosure above has
/// been explicitly acknowledged. Deterministic and UI-independent so the
/// safety invariant is provable without a live view.
public struct OwnedSiteCreationGate: Equatable {
    public var disclosureAcknowledged: Bool

    public init(disclosureAcknowledged: Bool = false) {
        self.disclosureAcknowledged = disclosureAcknowledged
    }

    /// False until the disclosure is acknowledged; flips back to false if the
    /// acknowledgement is withdrawn (e.g. the toggle is turned off again
    /// before confirming), so the gate can never be primed and left armed.
    public var canMint: Bool { disclosureAcknowledged }
}

/// The blocking §9.3 sheet. Presented before `createOwnedSite()` is ever
/// called; the mint button stays `.disabled` until the person acknowledges
/// the disclosure. Not wired to a reachable entry point yet — the owned
/// masthead write path and `follow_site` are deferred (Rung 5), so a minted
/// site is currently inert. This view is delivered ready for that flow to
/// adopt once it lands.
public struct OwnedSiteSeizureDisclosureView: View {
    @Environment(\.colorScheme) private var colorScheme
    @State private var gate = OwnedSiteCreationGate()
    private let onConfirm: () -> Void
    private let onCancel: () -> Void

    public init(
        onConfirm: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.onConfirm = onConfirm
        self.onCancel = onCancel
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(SiteSeizureDisclosure.title)
                    .font(.riot(.poster, size: 28, relativeTo: .largeTitle))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                Text(SiteSeizureDisclosure.body)
                    .font(.riot(.body, size: 17, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityIdentifier("seizure-disclosure")
                Toggle(isOn: $gate.disclosureAcknowledged) {
                    Text(SiteSeizureDisclosure.acknowledgementLabel)
                        .font(.riot(.body, size: 15, relativeTo: .body))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                }
                .accessibilityIdentifier("seizure-ack-toggle")
                Button("Create this site") { onConfirm() }
                    .buttonStyle(.riotPrimary)
                    .disabled(!gate.canMint)
                    .accessibilityIdentifier("owned-site-mint")
                Button("Not now") { onCancel() }
                    .buttonStyle(.riotSecondary)
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
    }
}
