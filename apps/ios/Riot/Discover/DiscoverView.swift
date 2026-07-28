import SwiftUI

// MARK: - Discover front door (browse communities) + Preview (look inside first)
//
// The Discover screen is the reachable answer to "find a community I don't
// already have a link for": a search field, category chips, a "bring your crew"
// import affordance, and a list of community cards. Tapping a card opens a
// Preview — steward, what it's about, a bounded "what's new" — whose primary
// action routes into the EXISTING join flow (`RiotAppModel.commitJoin` /
// join-by-reference). It does not reinvent joining.
//
// Every row that is a seed wears a "Sample" badge, so nothing here is presented
// as a genuine live listing while the signed-directory-feed backend is unbuilt.

/// The Discover screen. Reads communities through `CommunityDiscoveryModel`
/// (backed by the seeded source today, a live feed later) and routes joining
/// back to the app model.
public struct DiscoverView: View {
    @Environment(\.colorScheme) private var colorScheme
    @StateObject private var model: CommunityDiscoveryModel

    private let onJoin: (DiscoverableCommunity) -> Void
    private let onImportCrew: () -> Void
    private let onCreate: () -> Void
    private let onClose: () -> Void

    public init(
        source: CommunityDirectory = SeededCommunityDirectory(),
        onJoin: @escaping (DiscoverableCommunity) -> Void,
        onImportCrew: @escaping () -> Void,
        onCreate: @escaping () -> Void,
        onClose: @escaping () -> Void
    ) {
        _model = StateObject(wrappedValue: CommunityDiscoveryModel(source: source))
        self.onJoin = onJoin
        self.onImportCrew = onImportCrew
        self.onCreate = onCreate
        self.onClose = onClose
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    searchField
                    importCrewCard
                    categoryChips
                    resultsSection
                    Button("Start a community instead", action: onCreate)
                        .buttonStyle(.riotSecondary)
                        .accessibilityIdentifier("discover-start-community")
                }
                .padding(20)
            }
            .riotHeader(eyebrow: "Discover", "Find your people")
            .navigationDestination(for: DiscoverableCommunity.self) { community in
                CommunityPreviewView(community: community) {
                    onJoin(community)
                }
            }
            .safeAreaInset(edge: .bottom) {
                Button("Done", action: onClose)
                    .buttonStyle(.riotPrimary)
                    .accessibilityIdentifier("discover-done")
                    .padding(20)
            }
            .onAppear { model.refresh() }
        }
    }

    // MARK: - Search

    private var searchField: some View {
        RiotCard {
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                TextField("Search communities, people, places…", text: $model.searchText)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .textFieldStyle(.plain)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("discover-search-field")
                if !model.searchText.isEmpty {
                    Button {
                        model.searchText = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("discover-search-clear")
                }
            }
        }
    }

    // MARK: - Bring your crew / import

    /// The "networks start warm, not cold" affordance. It is deliberately a stub
    /// for now: importing a thread / Signal group / flyer QR routes to the
    /// existing paste-link/QR join sheet. A richer importer is a follow-up.
    private var importCrewCard: some View {
        Button(action: onImportCrew) {
            HStack(spacing: 12) {
                Image(systemName: "person.3.sequence.fill")
                    .font(.system(size: 20))
                    .foregroundStyle(RiotTheme.accent(for: colorScheme))
                VStack(alignment: .leading, spacing: 3) {
                    Text("Bring your crew")
                        .font(.riot(.body, size: 15, relativeTo: .headline))
                        .fontWeight(.semibold)
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    Text("Import the group you already have — a link or a flyer's QR. Networks start warm, not cold.")
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        .multilineTextAlignment(.leading)
                }
                Spacer(minLength: 0)
            }
        }
        .buttonStyle(.plain)
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(RiotTheme.accent(for: colorScheme).opacity(colorScheme == .dark ? 0.14 : 0.09))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(RiotTheme.line(for: colorScheme), lineWidth: 1)
        )
        .accessibilityIdentifier("discover-import-crew")
    }

    // MARK: - Category chips

    private var categoryChips: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Browse by what you need")
                .font(.riot(.mono, size: 12, relativeTo: .caption))
                .textCase(.uppercase)
                .tracking(1)
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(DiscoverCategory.allCases) { category in
                        categoryChip(category)
                    }
                }
            }
        }
    }

    private func categoryChip(_ category: DiscoverCategory) -> some View {
        let selected = model.selectedCategory == category
        return Button {
            model.toggle(category)
        } label: {
            HStack(spacing: 6) {
                Image(systemName: category.systemImage)
                    .font(.system(size: 12))
                Text(category.label)
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .fontWeight(.semibold)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .foregroundStyle(
                selected ? RiotTheme.onAccent(for: colorScheme) : RiotTheme.ink(for: colorScheme)
            )
            .background(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(selected ? RiotTheme.accent(for: colorScheme) : RiotTheme.card(for: colorScheme))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(RiotTheme.line(for: colorScheme), lineWidth: selected ? 0 : 1)
            )
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("discover-category-\(category.rawValue)")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    // MARK: - Results

    @ViewBuilder private var resultsSection: some View {
        Text(sectionTitle)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))

        if model.results.isEmpty {
            RiotCard {
                Text("No communities match that yet. Try a different word, or clear the filters.")
                    .font(.riot(.body, size: 14, relativeTo: .body))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    .accessibilityIdentifier("discover-empty")
            }
        } else {
            ForEach(model.results) { community in
                NavigationLink(value: community) {
                    DiscoverCommunityCard(community: community)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("discover-card-\(community.id)")
            }
            Text("Tap a community to look inside before joining")
                .font(.riot(.body, size: 12, relativeTo: .caption))
                .foregroundStyle(RiotTheme.accent(for: colorScheme))
                .frame(maxWidth: .infinity, alignment: .center)
        }
    }

    private var sectionTitle: String {
        if let category = model.selectedCategory {
            return "Communities — \(category.label)"
        }
        return "Communities you can find"
    }
}

/// One community card in the Discover list: monogram badge, name, one-line about,
/// open/invite + sample tags, and a steward + activity meta line.
struct DiscoverCommunityCard: View {
    @Environment(\.colorScheme) private var colorScheme
    let community: DiscoverableCommunity

    var body: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .top, spacing: 12) {
                    monogramBadge
                    VStack(alignment: .leading, spacing: 2) {
                        Text(community.name)
                            .font(.riotSerif(size: 19, relativeTo: .headline))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        Text("Kept by \(community.stewardName)")
                            .font(.riot(.body, size: 13, relativeTo: .caption))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    }
                    Spacer(minLength: 0)
                }
                Text(community.about)
                    .font(.riot(.body, size: 14, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .multilineTextAlignment(.leading)
                HStack(spacing: 8) {
                    tag(community.accessLabel, accent: community.isOpen)
                    if community.isSeed { tag("Sample", accent: false) }
                    Spacer(minLength: 0)
                    Text("\(community.peopleCount) people · \(community.activityHint)")
                        .font(.riot(.mono, size: 11, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
            }
        }
    }

    private var monogramBadge: some View {
        Text(community.monogram)
            .font(.riot(.monoBold, size: 15, relativeTo: .headline))
            .foregroundStyle(RiotTheme.onAccent(for: colorScheme))
            .frame(width: 42, height: 42)
            .background(
                RoundedRectangle(cornerRadius: 11, style: .continuous)
                    .fill(RiotTheme.avatarColor(forKey: community.id))
            )
    }

    private func tag(_ text: String, accent: Bool) -> some View {
        Text(text)
            .font(.riot(.mono, size: 10, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(0.5)
            .foregroundStyle(accent ? RiotTheme.accent(for: colorScheme) : RiotTheme.inkSoft(for: colorScheme))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .overlay(
                Capsule().strokeBorder(
                    accent ? RiotTheme.accent(for: colorScheme) : RiotTheme.line(for: colorScheme),
                    lineWidth: 1
                )
            )
    }
}

/// The Preview screen — look inside before joining. Leads with who keeps the
/// place and what it's about, shows a bounded "what's new" (never an endless
/// scroll), and offers a single primary join action that routes into the
/// existing join flow via the supplied `onJoin`.
public struct CommunityPreviewView: View {
    @Environment(\.colorScheme) private var colorScheme
    let community: DiscoverableCommunity
    let onJoin: () -> Void

    public init(community: DiscoverableCommunity, onJoin: @escaping () -> Void) {
        self.community = community
        self.onJoin = onJoin
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                aboutCard
                stewardCard
                whatsNewCard
                if community.isSeed {
                    Text("This is a sample listing. The live directory of signed community feeds is coming; joining routes through the same paste-a-link flow you already use.")
                        .font(.riot(.body, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        .accessibilityIdentifier("preview-sample-note")
                }
                Button(joinButtonTitle, action: onJoin)
                    .buttonStyle(.riotPrimary)
                    .accessibilityIdentifier("preview-join")
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Preview", community.name)
    }

    private var joinButtonTitle: String {
        community.isOpen ? "Join \(community.name)" : "Ask to join \(community.name)"
    }

    private var aboutCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    Text(community.accessLabel)
                        .font(.riot(.mono, size: 11, relativeTo: .caption))
                        .textCase(.uppercase)
                        .foregroundStyle(
                            community.isOpen
                                ? RiotTheme.accent(for: colorScheme)
                                : RiotTheme.inkSoft(for: colorScheme)
                        )
                    Text("·")
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    Text("\(community.peopleCount) people · \(community.activityHint)")
                        .font(.riot(.mono, size: 11, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                Text(community.about)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
            }
        }
    }

    private var stewardCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 6) {
                Text("Who keeps this place")
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .textCase(.uppercase)
                    .tracking(1)
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                HStack(spacing: 10) {
                    Text(stewardMonogram)
                        .font(.riot(.monoBold, size: 13, relativeTo: .body))
                        .foregroundStyle(RiotTheme.onAccent(for: colorScheme))
                        .frame(width: 36, height: 36)
                        .background(
                            Circle().fill(RiotTheme.avatarColor(forKey: community.stewardName))
                        )
                    VStack(alignment: .leading, spacing: 1) {
                        Text(community.stewardName)
                            .font(.riot(.body, size: 15, relativeTo: .headline))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        Text("Steward")
                            .font(.riot(.body, size: 12, relativeTo: .caption))
                            .foregroundStyle(RiotTheme.accent(for: colorScheme))
                    }
                }
            }
        }
    }

    @ViewBuilder private var whatsNewCard: some View {
        if !community.whatsNew.isEmpty {
            RiotCard {
                VStack(alignment: .leading, spacing: 8) {
                    Text("What's new")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textCase(.uppercase)
                        .tracking(1)
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    ForEach(Array(community.whatsNew.enumerated()), id: \.offset) { _, item in
                        HStack(alignment: .top, spacing: 8) {
                            Text("→")
                                .foregroundStyle(RiotTheme.accent(for: colorScheme))
                            Text(item)
                                .font(.riot(.body, size: 14, relativeTo: .body))
                                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        }
                    }
                    Text("A glimpse — not an endless scroll.")
                        .font(.riot(.body, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
            }
        }
    }

    private var stewardMonogram: String {
        let letters = community.stewardName.split(separator: " ").prefix(2).compactMap { $0.first }
        return String(letters).uppercased()
    }
}

// MARK: - App model wiring
//
// The Discover surface is presented from the community chooser and the launch
// screen. These helpers keep that wiring in one place and route joining through
// the EXISTING join paths on `RiotAppModel` — `commitJoin` for a real reference,
// the paste/QR sheet for a seed that carries none.

public extension RiotAppModel {
    /// Chooser / launch "Discover communities": close the chooser and present the
    /// Discover front door. Mirrors `requestJoinByReference`.
    func requestDiscover() {
        isCommunityChooserPresented = false
        isDiscoverPresented = true
    }

    /// Dismisses the Discover sheet.
    func dismissDiscover() { isDiscoverPresented = false }

    /// Routes a Preview's join action into the existing join flow. A community
    /// carrying a real reference is decoded and committed through `commitJoin`
    /// (join-or-switch, no duplicate row); a seed with no reference honestly falls
    /// back to the paste/QR sheet rather than fabricating a joinable coordinate.
    /// Closes the Discover sheet on a committed join.
    func joinDiscovered(_ community: DiscoverableCommunity) {
        switch CommunityJoinRoute.route(for: community) {
        case let .commitReference(reference):
            if let preview = try? JoinReferenceModel().preview(fromPastedString: reference) {
                commitJoin(preview: preview)
                if errorMessage == nil { isDiscoverPresented = false }
            } else {
                // A reference that won't decode should not dead-end — offer paste/QR.
                isDiscoverPresented = false
                isJoinByReferencePresented = true
            }
        case .pasteOrScan:
            isDiscoverPresented = false
            isJoinByReferencePresented = true
        }
    }
}
