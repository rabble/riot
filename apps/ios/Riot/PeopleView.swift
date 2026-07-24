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
    public static let title = "Known contributors"
    public static let organizerBadge = "Organizer"
    public static let emptyTitle = "No known contributors yet"
    public static let emptyMessage =
        "Known contributors appear here once people post updates."
    public static let emptyActionLabel = "Post the first update"
    public static let unavailableMessage =
        "This community's contributors are unavailable right now. Try again."

    /// The Person detail surface — a contributor's page. The eyebrow names what
    /// the page IS (a contributor, derived from signed records), never a member.
    public static let personEyebrow = "Contributor"
    public static let personPostsTitle = "Posts"
    /// The person has posted nothing this device can see yet — an honest empty
    /// state, never a fabricated row. A person known only through replies or
    /// editorial actions lands here.
    public static let personNoPostsTitle = "No posts from this person yet"
    public static let personNoPostsMessage =
        "Posts this person signed appear here once they arrive on your device."
    public static let personUnavailableMessage =
        "This person's posts are unavailable right now. Try again."

    /// The count phrase for a row — content-derived, never presence. Singular
    /// and plural so "1 contribution" never reads as "1 contributions".
    public static func contributions(_ count: UInt32) -> String {
        count == 1 ? "1 contribution" : "\(count) contributions"
    }

    /// The person page's contribution summary, naming the community when we know
    /// it (#4: "12 contributions in Rojava Solidarity"). A blank name falls back
    /// to the bare count — we never render "in " with nothing after it.
    public static func contributions(_ count: UInt32, in community: String) -> String {
        let base = contributions(count)
        let trimmed = community.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? base : "\(base) in \(trimmed)"
    }

    /// The recency line shown when this person holds (or ties) the community's
    /// freshest visible update. Honest, ordering-domain phrasing — not a wall
    /// clock the app cannot derive.
    public static let mostRecentToPost = "Most recent to post here"

    /// Section subtitle for the posts list — how many of this person's posts this
    /// device can actually see, distinct from the total contribution count.
    public static func postsShown(_ count: Int) -> String {
        count == 1 ? "1 post on this device" : "\(count) posts on this device"
    }
}

// MARK: - Recent activity: an honest recency, kept in the ordering domain

/// Recency phrasing for a contributor's page.
///
/// A projected post now carries a real creation instant (`createdAtUnixSeconds`,
/// recovered by core from the Willow entry timestamp — see the FFI projection),
/// so recency is a true wall-clock "ago" against the current clock, not the old
/// ordering-domain gap. Every function is pure over a creation instant and an
/// explicit `now`, so it stays deterministic under test. When the person has no
/// visible posts — or core supplied no creation time — there is simply no line
/// (absence, never a fabricated "active now").
public enum PersonActivity {
    /// The header recency line ("Last posted 2h ago"), or `nil` when the person
    /// has no visible posts / no recoverable creation time. Derived from the real
    /// creation instant of their newest post.
    public static func headerRecency(
        personNewestCreatedUnixSeconds: UInt64?,
        now: Date = Date()
    ) -> String? {
        guard let ago = RelativeTime.ago(unixSeconds: personNewestCreatedUnixSeconds, now: now)
        else { return nil }
        return "Last posted \(ago)"
    }

    /// The per-row recency caption — a true "2h ago" from the row's own creation
    /// instant. `nil` when core supplied no time, so the row omits the caption
    /// rather than showing a bogus instant.
    public static func rowRecency(rowCreatedUnixSeconds: UInt64?, now: Date = Date()) -> String? {
        RelativeTime.ago(unixSeconds: rowCreatedUnixSeconds, now: now)
    }
}

public struct PersonRowAccessibilityValue: Equatable, Sendable {
    public let label: String
}

public enum PersonRowAccessibility {
    public static func summary(_ row: PersonRow) -> PersonRowAccessibilityValue {
        PersonRowAccessibilityValue(label: row.accessibilityLabel)
    }

    public static func technicalLabel(_ row: PersonRow) -> String {
        "Technical details for \(row.rendered)"
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
    private let onSelectPerson: (PersonRow) -> Void
    private let composerFocus: FocusState<ComposerOrigin?>.Binding
    @Environment(\.colorScheme) private var colorScheme

    public init(
        model: PeopleSurfaceModel,
        onPostUpdate: @escaping () -> Void,
        onSelectPerson: @escaping (PersonRow) -> Void = { _ in },
        composerFocus: FocusState<ComposerOrigin?>.Binding
    ) {
        self.model = model
        self.onPostUpdate = onPostUpdate
        self.onSelectPerson = onSelectPerson
        self.composerFocus = composerFocus
    }

    public var body: some View {
        Group {
            switch model.state {
            case let .populated(rows):
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(rows) { row in
                            // The row becomes a path INTO the person: tapping it
                            // opens their page (who they are + what they posted),
                            // turning the roster from a dead end into a way to
                            // reach a contributor's content.
                            Button { onSelectPerson(row) } label: {
                                RiotCard { PersonRowView(row: row) }
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("person-row-\(row.id)")
                            .accessibilityHint("Opens this contributor's posts")
                        }
                    }
                    .padding(20)
                }
            case let .empty(empty):
                emptyState(empty)
            case let .unavailable(message):
                unavailableState(message)
            }
        }
        .riotHeader(eyebrow: "Community", PeopleStrings.title)
        .onAppear { model.load() }
    }

    private func emptyState(_ empty: EmptyPeopleState) -> some View {
        ScrollView {
            RiotCard {
                VStack(alignment: .leading, spacing: 12) {
                    Text(empty.title)
                        .font(.riot(.body, size: 17, relativeTo: .headline))
                    Text(empty.message)
                        .font(.riot(.body, size: 15, relativeTo: .callout))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    Button(empty.actionLabel, action: onPostUpdate)
                        .buttonStyle(.riotPrimary)
                        .frame(minHeight: 44)
                        .focused(composerFocus, equals: .people)
                        .accessibilityIdentifier("people-post-first-update")
                }
            }
            .padding(20)
        }
    }

    private func unavailableState(_ message: String) -> some View {
        ScrollView {
            RiotCard {
                VStack(alignment: .leading, spacing: 12) {
                    Text(message)
                        .font(.riot(.body, size: 15, relativeTo: .callout))
                    Button("Try again") { model.load() }
                        .buttonStyle(.riotPrimary)
                        .frame(minHeight: 44)
                        .accessibilityIdentifier("people-retry")
                }
            }
            .padding(20)
        }
    }
}

/// One contributor row, now a summary that leads INTO the person's page. The
/// name is the rendered string and the organizer badge is text; the raw hex id
/// no longer sits on the roster — it moves to the person's detail page, where
/// their content lives. A chevron signals the row is a way in, not a dead end.
private struct PersonRowView: View {
    let row: PersonRow
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        HStack(spacing: 12) {
            PersonAvatar(displayName: row.displayName, isOrganizer: row.isOrganizer, keySeed: row.id)
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(row.rendered)
                        .font(.riot(.body, size: 17, relativeTo: .headline))
                    if row.isOrganizer {
                        RiotBadge(PeopleStrings.organizerBadge)
                    }
                    Spacer(minLength: 0)
                }
                Text(PeopleStrings.contributions(row.contributionCount))
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
            Image(systemName: "chevron.right")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
        .frame(minHeight: 44)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(PersonRowAccessibility.summary(row).label)
        .accessibilityAddTraits(.isButton)
    }
}

/// A person's avatar: their initials on a tinted disc (the organizer's disc is
/// pink so the mark reads at a glance), never a fetched image — this is a P2P
/// app with no avatar hosting. Initials are derived from the sanitized display
/// name the core already produced.
struct PersonAvatar: View {
    let displayName: String
    var isOrganizer: Bool = false
    var diameter: CGFloat = 40
    /// The author's unique key (hex id or short tag), used to derive the initials
    /// when there is no real display name — so two nameless members never both
    /// read "ME". `nil` keeps the old name-only initials for callers that have no
    /// key to hand.
    var keySeed: String?
    /// An explicit, key-derived disc colour (see ``RiotTheme/avatarColor(forKey:)``).
    /// `nil` keeps the roster's default disc, so existing People rows are unchanged;
    /// the newswire byline passes one so each author reads as a distinct face.
    var tint: Color?
    @Environment(\.colorScheme) private var colorScheme

    private var discColor: Color {
        if let tint { return tint }
        return isOrganizer ? RiotTheme.pink(for: colorScheme) : RiotTheme.paper2(for: colorScheme)
    }

    private var glyphColor: Color {
        (isOrganizer || tint != nil)
            ? RiotTheme.onAccent(for: colorScheme)
            : RiotTheme.ink(for: colorScheme)
    }

    /// Up to two initials from the display name; a single glyph for a one-word
    /// name, and a bullet if the name is empty (a nameless author still gets a
    /// stable disc, never a blank).
    static func initials(for name: String) -> String {
        let words = name.split(whereSeparator: { $0 == " " || $0 == "·" })
            .filter { !$0.isEmpty }
        guard let first = words.first else { return "•" }
        if words.count >= 2, let last = words.last {
            return String(first.prefix(1) + last.prefix(1)).uppercased()
        }
        return String(first.prefix(2)).uppercased()
    }

    /// The initials to draw for an author. A real claimed name uses its own
    /// initials. Core's fallback for a nameless peer is the bare word "member"
    /// (surfaced friendly as "Member") — taking initials from THAT would stamp
    /// "ME" on every nameless author, so instead derive two glyphs from the
    /// author's unique key. The key tag is hex, so this echoes the tag shown
    /// beside the name and is distinct per person; the disc colour is already
    /// key-derived, so the two never collide together.
    static func initials(displayName: String, keySeed: String?) -> String {
        let trimmed = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty, trimmed.lowercased() != "member" {
            return initials(for: trimmed)
        }
        guard let keySeed else { return initials(for: displayName) }
        let hex = keySeed.filter { $0.isHexDigit }
        guard !hex.isEmpty else { return "•" }
        return String(hex.prefix(2)).uppercased()
    }

    var body: some View {
        Text(Self.initials(displayName: displayName, keySeed: keySeed))
            .font(.riot(.body, size: diameter * 0.36, relativeTo: .headline))
            .fontWeight(.semibold)
            .foregroundStyle(glyphColor)
            .frame(width: diameter, height: diameter)
            .background(Circle().fill(discColor))
            .accessibilityHidden(true)
    }
}

// MARK: - Person detail: a contributor's path to their content

/// The pure filter behind a person's page: every post THIS person authored in
/// the community, drawn from the SAME collective projection the Home wire draws,
/// then narrowed to one author id. It never re-decides ordering or treatment —
/// it selects and de-duplicates. A post featured on both the front page and the
/// open wire is returned once; expired ("earlier") posts still count as this
/// person's content. Newest first by the signed ordering value.
public enum PersonPosts {
    public static func authored(
        by personID: String,
        in projection: NewswireProjectionView
    ) -> [NewswirePostRow] {
        let all = projection.frontPage + projection.openWire + projection.earlier
        var seen = Set<String>()
        var rows: [NewswirePostRow] = []
        for post in all where post.author.id == personID {
            guard seen.insert(post.entryId).inserted else { continue }
            rows.append(NewswirePostRow(post))
        }
        return rows.sorted { $0.taiJ2000Micros > $1.taiJ2000Micros }
    }
}

/// What the person's page is showing. A person with no visible posts is an
/// honest empty state, never a fabricated row; a projection failure is a fixed
/// message, never a raw internal error.
public enum PersonDetailState: Equatable, Sendable {
    case posts([NewswirePostRow])
    case empty
    case unavailable(String)
}

/// Loads a contributor's page: their identity (carried in from the roster row)
/// plus the posts they authored, filtered from the community's newswire
/// projection. Reuses the existing `NewswireProjecting` seam — no new FFI, the
/// same projection Home already draws.
@MainActor
public final class PersonDetailModel: ObservableObject {
    @Published public private(set) var state: PersonDetailState
    /// The header recency line, derived on `load()` from the newest of this
    /// person's posts against the community's freshest visible update. `nil`
    /// whenever there is nothing to show — no posts, or a projection failure — so
    /// the header never renders a fabricated "active" line (see `PersonActivity`).
    @Published public private(set) var recentActivity: String?
    /// The freshest visible ordering value across the whole community projection,
    /// captured on `load()` so each post row can show its recency against the same
    /// anchor the header uses. `nil` before a successful load.
    @Published public private(set) var communityNewestMicros: UInt64?
    public let person: PersonRow

    private let projector: NewswireProjecting
    private let spaceDescriptorEntryID: String
    /// The community this page is being viewed inside, for the contribution
    /// summary (#4). Empty when unknown — the summary then omits it cleanly.
    private let communityName: String
    /// The clock the header recency reads "ago" against. Injectable so recency is
    /// deterministic under test; production defaults to the live clock.
    private let now: () -> Date

    public init(
        person: PersonRow,
        projector: NewswireProjecting,
        spaceDescriptorEntryID: String,
        communityName: String = "",
        initialState: PersonDetailState = .empty,
        now: @escaping () -> Date = { Date() }
    ) {
        self.person = person
        self.projector = projector
        self.spaceDescriptorEntryID = spaceDescriptorEntryID
        self.communityName = communityName
        self.state = initialState
        self.now = now
    }

    /// The contribution summary shown in the header: the core-derived contribution
    /// count, named to the community when we know which one this is.
    public var contributionSummary: String {
        PeopleStrings.contributions(person.contributionCount, in: communityName)
    }

    public func load() {
        do {
            let projection = try projector.projectNewswire(
                spaceDescriptorEntryID: spaceDescriptorEntryID
            )
            let rows = PersonPosts.authored(by: person.id, in: projection)
            // The freshest ordering value the whole community shows, retained for
            // callers that reason about ordering. Recency itself is now a true
            // wall-clock "ago" from each post's real creation time.
            let allPosts = projection.frontPage + projection.openWire + projection.earlier
            communityNewestMicros = allPosts.map(\.taiJ2000Micros).max()
            // The person's newest post by ordering; show its real creation time as
            // "Last posted N ago". `rows` is already newest-first by ordering.
            recentActivity = PersonActivity.headerRecency(
                personNewestCreatedUnixSeconds: rows.first?.createdAtUnixSeconds,
                now: now()
            )
            state = rows.isEmpty ? .empty : .posts(rows)
        } catch {
            // Drop the underlying error: a fixed, actionable message, never
            // internal detail (§4.7), exactly as the People surface does. No
            // recency line survives a failure.
            recentActivity = nil
            communityNewestMicros = nil
            state = .unavailable(PeopleStrings.personUnavailableMessage)
        }
    }
}

/// A contributor's page: who they are + what they posted. Each post row is a tap
/// into the EXISTING newswire report detail (`NewswireReportDetailSheet`), driven
/// by the community's shared surface model — the same sheet the Home wire opens,
/// never a rebuilt one. This is the path People was missing: roster → person →
/// their content → a post.
public struct PersonDetailView: View {
    @ObservedObject private var model: PersonDetailModel
    @ObservedObject private var surfaceModel: NewswireSurfaceModel
    private let onClose: () -> Void
    @State private var reading: NewswirePostRow?
    @Environment(\.colorScheme) private var colorScheme

    public init(
        model: PersonDetailModel,
        surfaceModel: NewswireSurfaceModel,
        onClose: @escaping () -> Void
    ) {
        self.model = model
        self.surfaceModel = surfaceModel
        self.onClose = onClose
    }

    private var person: PersonRow { model.person }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                header
                switch model.state {
                case let .posts(rows):
                    postsSection(rows)
                case .empty:
                    emptyState
                case let .unavailable(message):
                    unavailableState(message)
                }
                Button("Close", action: onClose)
                    .buttonStyle(.riotSecondary)
                    .frame(maxWidth: .infinity, minHeight: 44)
                    .accessibilityIdentifier("person-detail-close")
            }
            .padding(20)
        }
        .riotHeader(eyebrow: PeopleStrings.personEyebrow, person.displayName)
        .onAppear { model.load() }
        .sheet(item: $reading) { row in
            NewswireReportDetailSheet(
                model: surfaceModel,
                row: row,
                onClose: { reading = nil }
            )
        }
    }

    private var header: some View {
        RiotCard {
            HStack(spacing: 14) {
                PersonAvatar(
                    displayName: person.displayName,
                    isOrganizer: person.isOrganizer,
                    diameter: 56,
                    keySeed: person.id
                )
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(person.rendered)
                            .font(.riot(.body, size: 20, relativeTo: .title3))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        if person.isOrganizer {
                            RiotBadge(PeopleStrings.organizerBadge)
                        }
                    }
                    // What you most want to know first: is this person current or
                    // historical? Shown only when we can derive it honestly from
                    // their newest post; absent otherwise (never a fake "active").
                    if let recentActivity = model.recentActivity {
                        Text(recentActivity)
                            .font(.riot(.body, size: 13, relativeTo: .footnote))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                            .accessibilityIdentifier("person-detail-recent-activity")
                    }
                    Text(model.contributionSummary)
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    DisclosureGroup("Technical details") {
                        Text(verbatim: person.id)
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textSelection(.enabled)
                            .accessibilityIdentifier("person-detail-id")
                    }
                    .accessibilityLabel(PersonRowAccessibility.technicalLabel(person))
                }
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(PersonRowAccessibility.summary(person).label)
        }
    }

    private func postsSection(_ rows: [NewswirePostRow]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                Text(PeopleStrings.personPostsTitle)
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .textCase(.uppercase)
                    .tracking(0.5)
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                Spacer(minLength: 0)
                // How many of this person's posts this device can actually see —
                // distinct from the total contribution count in the header.
                Text(PeopleStrings.postsShown(rows.count))
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    .accessibilityIdentifier("person-detail-posts-shown")
            }
            ForEach(rows) { row in
                Button { reading = row } label: {
                    RiotCard {
                        PersonPostRowView(row: row)
                    }
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("person-post-\(row.id)")
                .accessibilityLabel(row.readAccessibilityLabel)
            }
        }
    }

    private var emptyState: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 8) {
                Text(PeopleStrings.personNoPostsTitle)
                    .font(.riot(.body, size: 17, relativeTo: .headline))
                Text(PeopleStrings.personNoPostsMessage)
                    .font(.riot(.body, size: 15, relativeTo: .callout))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
        }
        .accessibilityIdentifier("person-detail-empty")
    }

    private func unavailableState(_ message: String) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text(message)
                    .font(.riot(.body, size: 15, relativeTo: .callout))
                Button("Try again") { model.load() }
                    .buttonStyle(.riotPrimary)
                    .frame(minHeight: 44)
                    .accessibilityIdentifier("person-detail-retry")
            }
        }
    }
}

/// One of a person's posts, ready to draw on their page: the headline (or the
/// treatment copy when the post is hidden/tombstoned), a short body preview, and
/// the trust markers the wire shows. A compact echo of the open-wire row, tuned
/// for a single-author list.
private struct PersonPostRowView: View {
    let row: NewswirePostRow
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            // Recency first, so the list scans as a timeline of this person's
            // activity. A true "2h ago" from the post's real creation time (see
            // `PersonActivity`); omitted only when core supplied no time.
            if let ago = PersonActivity.rowRecency(rowCreatedUnixSeconds: row.createdAtUnixSeconds) {
                Text(ago)
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    .accessibilityIdentifier("person-post-recency-\(row.id)")
            }
            switch row.display {
            case .ordinary:
                Text(verbatim: row.headline ?? "Update")
                    .font(.riot(.body, size: 17, relativeTo: .headline))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                if let body = row.body, !body.isEmpty {
                    Text(verbatim: body)
                        .font(.riot(.body, size: 15, relativeTo: .body))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        .lineLimit(2)
                }
                if let when = eventTimeText {
                    Text("Event · \(when)")
                        .font(.riot(.mono, size: 11, relativeTo: .caption2))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                if row.hasCorrection {
                    RiotBadge(EditorialCorrectionLabel.text)
                }
                if row.verificationCount > 0 {
                    Text("Editorial checks: \(row.verificationCount)")
                        .font(.riot(.mono, size: 11, relativeTo: .caption2))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                if row.aiAssisted {
                    RiotBadge(NewswireTrustCopy.aiAssisted)
                }
            case .hiddenInterstitial:
                Text(NewswireTreatmentCopy.hiddenTitle)
                    .font(.riot(.body, size: 17, relativeTo: .headline))
                Text(NewswireTreatmentCopy.hiddenBody)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            case .tombstoned:
                Text(NewswireTreatmentCopy.tombstoneTitle)
                    .font(.riot(.body, size: 17, relativeTo: .headline))
                Text(NewswireTreatmentCopy.tombstoneBody)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
            HStack {
                Spacer(minLength: 0)
                Text("Read update")
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                    .foregroundStyle(RiotTheme.pink(for: colorScheme))
                Image(systemName: "chevron.right")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(RiotTheme.pink(for: colorScheme))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var eventTimeText: String? {
        row.eventTimeUnixSeconds.map {
            Date(timeIntervalSince1970: TimeInterval($0))
                .formatted(date: .abbreviated, time: .shortened)
        }
    }
}
