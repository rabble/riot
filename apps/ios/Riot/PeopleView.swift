import SwiftUI

// MARK: - Projector seam

/// The one call the People surface needs: the community's Known-contributors,
/// projected from its signed records. `RiotProfileRepository` conforms to this
/// (see its Newswire extension); tests inject a stub so the surface is exercised
/// without a store or the FFI.
public protocol NewswireContributorProjecting {
    func projectNewswireContributors(spaceDescriptorEntryID: String) throws -> [NewswireContributor]
}

// MARK: - Surface vocabulary

/// Every fixed string the People surface shows. Isolated so the anti-membership
/// contract can assert on the surface's own vocabulary directly: this is the
/// KNOWN CONTRIBUTORS of a community — the people behind its signed record — not
/// a membership roster and not presence. Nothing here says "member", "online",
/// or "present"; the fallback name "member · tag" a nameless author renders to
/// is a NAME, produced by the core resolver, never a membership label.
public enum PeopleStrings {
    public static let title = "Contributors"
    public static let organizerBadge = "Organizer"
    public static let emptyTitle = "No contributors yet"
    public static let emptyMessage =
        "Contributors appear here once people post updates. Be the first."
    public static let emptyActionLabel = "Post the first update"
    public static let unavailableMessage =
        "This community's contributors are unavailable right now. Try again."

    /// The count phrase for a row — content-derived, never presence. Singular
    /// and plural so "1 contribution" never reads as "1 contributions".
    public static func contributions(_ count: UInt32) -> String {
        count == 1 ? "1 contribution" : "\(count) contributions"
    }
}

// MARK: - Row

/// One known contributor, ready to draw. The display string is always the
/// resolver-rendered `name · tag`; the raw hex `id` is carried only for pinning
/// and the Technical-details disclosure, never shown as the name.
public struct PersonRow: Equatable, Identifiable, Sendable {
    /// The stable hex subspace id — for pinning and Technical details only.
    public let id: String
    /// The sanctioned display string, e.g. `Ana · a3f91122`.
    public let rendered: String
    public let displayName: String
    public let tag: String
    /// True ONLY when the core marked this author the recognized organizer
    /// (the namespace coordinate). The surface never derives it from a name.
    public let isOrganizer: Bool
    public let contributionCount: UInt32

    public init(_ contributor: NewswireContributor) {
        self.id = contributor.author.id
        self.rendered = contributor.author.rendered
        self.displayName = contributor.author.displayName
        self.tag = contributor.author.tag
        self.isOrganizer = contributor.isOrganizer
        self.contributionCount = contributor.contributionCount
    }

    /// A single spoken line: the rendered name, whether they organize, and how
    /// much they have contributed — organizer conveyed as WORDS, never color
    /// alone (§4.6).
    public var accessibilityLabel: String {
        var parts = [rendered]
        if isOrganizer { parts.append(PeopleStrings.organizerBadge) }
        parts.append(PeopleStrings.contributions(contributionCount))
        return parts.joined(separator: ", ")
    }
}

// MARK: - Empty state

/// The actionable empty state — never a blank list. Carries a call to action so
/// a community with no contributors yet still tells the reader what to do.
public struct EmptyPeopleState: Equatable, Sendable {
    public let title: String
    public let message: String
    public let actionLabel: String

    public static let noContributors = EmptyPeopleState(
        title: PeopleStrings.emptyTitle,
        message: PeopleStrings.emptyMessage,
        actionLabel: PeopleStrings.emptyActionLabel
    )
}

// MARK: - State

/// What the People surface is showing. There is no "loading roster" or presence
/// state — the surface is a pure projection of signed records.
public enum PeopleSurfaceState: Equatable, Sendable {
    case populated([PersonRow])
    case empty(EmptyPeopleState)
    /// A fixed, human message — never a raw internal error (§4.7).
    case unavailable(String)

    /// Builds the surface from projected contributors. No contributors is the
    /// actionable empty state, never `.populated([])`.
    public static func from(_ contributors: [NewswireContributor]) -> PeopleSurfaceState {
        let rows = contributors.map(PersonRow.init)
        return rows.isEmpty ? .empty(.noContributors) : .populated(rows)
    }
}

// MARK: - Model

/// Loads the People surface from a projector, mapping any failure to a fixed
/// message so a raw internal error never reaches the screen.
@MainActor
public final class PeopleSurfaceModel: ObservableObject {
    @Published public private(set) var state: PeopleSurfaceState

    private let projector: NewswireContributorProjecting
    private let spaceDescriptorEntryID: String

    public init(
        projector: NewswireContributorProjecting,
        spaceDescriptorEntryID: String,
        initialState: PeopleSurfaceState = .empty(.noContributors)
    ) {
        self.projector = projector
        self.spaceDescriptorEntryID = spaceDescriptorEntryID
        self.state = initialState
    }

    public func load() {
        do {
            let contributors = try projector.projectNewswireContributors(
                spaceDescriptorEntryID: spaceDescriptorEntryID
            )
            state = .from(contributors)
        } catch {
            // Deliberately drop the underlying error: the reader gets a fixed,
            // actionable message, never internal detail.
            state = .unavailable(PeopleStrings.unavailableMessage)
        }
    }
}

// MARK: - View

/// The People surface: the community's known contributors, organizer first.
public struct PeopleView: View {
    @ObservedObject private var model: PeopleSurfaceModel
    private let onPostUpdate: () -> Void
    private let composerFocus: FocusState<ComposerOrigin?>.Binding

    public init(
        model: PeopleSurfaceModel,
        onPostUpdate: @escaping () -> Void,
        composerFocus: FocusState<ComposerOrigin?>.Binding
    ) {
        self.model = model
        self.onPostUpdate = onPostUpdate
        self.composerFocus = composerFocus
    }

    public var body: some View {
        Group {
            switch model.state {
            case let .populated(rows):
                List(rows) { row in
                    PersonRowView(row: row)
                }
            case let .empty(empty):
                emptyState(empty)
            case let .unavailable(message):
                unavailableState(message)
            }
        }
        .navigationTitle(PeopleStrings.title)
        .onAppear { model.load() }
    }

    private func emptyState(_ empty: EmptyPeopleState) -> some View {
        VStack(spacing: 12) {
            Text(empty.title).font(.headline)
            Text(empty.message)
                .font(.subheadline)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            Button(empty.actionLabel, action: onPostUpdate)
                .frame(minHeight: 44)
                .focused(composerFocus, equals: .people)
        }
        .padding()
    }

    private func unavailableState(_ message: String) -> some View {
        VStack(spacing: 12) {
            Text(message)
                .font(.subheadline)
                .multilineTextAlignment(.center)
            Button("Try again") { model.load() }
                .frame(minHeight: 44)
        }
        .padding()
    }
}

/// One contributor row. The name is the rendered string; the organizer badge is
/// text, and the full id sits behind a Technical-details disclosure.
private struct PersonRowView: View {
    let row: PersonRow
    @State private var showsTechnicalDetails = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(row.rendered).font(.body)
                if row.isOrganizer {
                    Text(PeopleStrings.organizerBadge)
                        .font(.caption)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Capsule().fill(Color.accentColor.opacity(0.2)))
                }
                Spacer()
            }
            Text(PeopleStrings.contributions(row.contributionCount))
                .font(.caption)
                .foregroundStyle(.secondary)
            DisclosureGroup("Technical details", isExpanded: $showsTechnicalDetails) {
                Text(row.id)
                    .font(.caption.monospaced())
                    .textSelection(.enabled)
            }
        }
        .frame(minHeight: 44)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(row.accessibilityLabel)
    }
}
