import SwiftUI

// MARK: - Model

/// Whether a guide task works with no connectivity at all, or needs a system
/// permission or connection first. The design's "Works offline" contract: every
/// instruction says which, so a person in the field never starts a task that
/// cannot finish where they are.
public enum GuideConnectivity: Equatable, Sendable {
    case worksOffline
    case needsPermission(String)

    public var label: String {
        switch self {
        case .worksOffline: "Works offline"
        case let .needsPermission(what): "Needs \(what)"
        }
    }
}

/// One task in the field manual: a plain-language goal, numbered steps that
/// quote visible labels exactly, the expected result, and one recovery path.
public struct UsingRiotGuideTask: Equatable, Sendable, Identifiable {
    public let id: String
    public let goal: String
    public let steps: [String]
    public let connectivity: GuideConnectivity
    public let expectedResult: String
    public let recovery: String

    init(
        id: String,
        goal: String,
        steps: [String],
        connectivity: GuideConnectivity,
        expectedResult: String,
        recovery: String
    ) {
        self.id = id
        self.goal = goal
        self.steps = steps
        self.connectivity = connectivity
        self.expectedResult = expectedResult
        self.recovery = recovery
    }
}

/// One section of the manual. `notes` carry orientation or boundary prose that
/// is not a numbered task (Start here, Privacy, Platform notes, gaps).
public struct UsingRiotGuideSection: Equatable, Sendable, Identifiable {
    public let id: String
    public let title: String
    public let notes: [String]
    public let tasks: [UsingRiotGuideTask]

    init(id: String, title: String, notes: [String] = [], tasks: [UsingRiotGuideTask] = []) {
        self.id = id
        self.title = title
        self.notes = notes
        self.tasks = tasks
    }
}

/// The in-app "Using Riot" field manual (offline-guides design, Guide 2). It
/// documents only the current UI of this build — every quoted label is the
/// exact string the screen draws, and `UsingRiotGuideTests` pins the coupling.
/// It teaches no future architecture; gaps live in "What is not available yet".
public enum UsingRiotGuide {
    /// The canonical cross-platform entry label.
    public static let entryLabel = "Using Riot"

    /// When these instructions were last checked against the visible UI.
    /// Update whenever a quoted label or flow changes.
    public static let checkedDate = "2026-07-22"

    public static let sections: [UsingRiotGuideSection] = [
        UsingRiotGuideSection(
            id: "start-here",
            title: "Start here",
            notes: [
                "Riot keeps your communities, posts, and tools on this device. Reading, writing, switching communities, and using approved tools all work with no internet connection.",
                "Riot is a prototype. Screens and labels can change between builds; this guide matches the build it shipped with (checked \(checkedDate)).",
                "Updates travel two ways today: directly between nearby devices, and as an exported public web view. There is no central server your community depends on.",
                "Everything in a public community is public. Anyone who receives a post can keep it and pass it on — post accordingly.",
            ]
        ),

        UsingRiotGuideSection(
            id: "create-or-join",
            title: "Create or join a community",
            tasks: [
                UsingRiotGuideTask(
                    id: "create",
                    goal: "Create your own community",
                    steps: [
                        "On the Welcome screen, tap “Get started”.",
                        "Enter your name if you want one — it is optional, saved on this device, and shared with future peers.",
                        "Tap “Create a community”. (Already in a community? Open “Your communities” from the community name at the top, then tap “Create a community” there.)",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "The new community opens with its four screens: Home, Tools, People, and Nearby. You are its organizer.",
                    recovery: "If the new community does not open, close and reopen Riot — your profile and communities are saved on this device."
                ),
                UsingRiotGuideTask(
                    id: "join-link",
                    goal: "Join with a link someone shared",
                    steps: [
                        "Tap “Join with a link or QR” — on the Welcome screen, or in “Your communities”.",
                        "Choose “Paste” and paste the link.",
                        "Review the community name and identifier shown before joining.",
                        "Tap “Join this community”.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "The community is added. Its name and posts arrive on first sync — usually the first nearby exchange with someone who has them.",
                    recovery: "“That isn't a Riot community link.” means part of the link is missing — go back to where it was shared and copy all of it."
                ),
                UsingRiotGuideTask(
                    id: "join-qr",
                    goal: "Join by scanning a QR code",
                    steps: [
                        "Tap “Join with a link or QR”, then choose “Scan”.",
                        "Point the camera at the community's QR code.",
                        "Review the community name and identifier, then tap “Join this community”.",
                    ],
                    connectivity: .needsPermission("camera permission"),
                    expectedResult: "The community is added, the same as joining with a pasted link.",
                    recovery: "If the camera is blocked, tap “Open Settings” to allow it — or tap “Paste a link instead”."
                ),
                UsingRiotGuideTask(
                    id: "join-nearby",
                    goal: "Join from a nearby device",
                    steps: [
                        "In “Your communities”, tap “Find one nearby” — or open the Nearby screen in any community.",
                        "Tap “Find nearby devices” and pick the other person's device.",
                        "Accept the connection when asked. If they are in a different community, Riot asks before you join it — nothing changes without your confirmation.",
                        "Preview what they offer, then add it or tap “Not now”.",
                    ],
                    connectivity: .needsPermission("local-network or Bluetooth permission"),
                    expectedResult: "You join their community and carry a copy of the updates you accepted.",
                    recovery: "If no device appears, see “Exchange nearby” below — both devices must be close, on the Nearby screen, and allowed to use the local network."
                ),
            ]
        ),

        UsingRiotGuideSection(
            id: "manage",
            title: "Manage your communities",
            notes: [
                "Roles you may see on a community: Organizer, Member, Public reader, Following, and Personal space. The organizer can approve tools and take editorial actions; everyone can read and post on the open wire.",
                "Archiving and restoring communities is not surfaced on this platform yet — leaving is the only removal, and it is destructive on this device.",
            ],
            tasks: [
                UsingRiotGuideTask(
                    id: "switch",
                    goal: "Switch to another community",
                    steps: [
                        "Tap the community name at the top of the screen to open “Your communities”.",
                        "Tap the community you want.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "That community opens where you left it. Your posting identity is separate in each community by design.",
                    recovery: "A community marked “Needs recovery before it can open.” offers Retry — Riot quarantines rather than deletes, so retrying is safe."
                ),
                UsingRiotGuideTask(
                    id: "leave",
                    goal: "Leave a community",
                    steps: [
                        "Open the community, then tap the settings symbol at the top to open “Community settings”.",
                        "Tap “Leave this community” and confirm.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "The community is removed from this device. Copies other devices already carry are not affected.",
                    recovery: "If you leave by mistake, rejoin with the community's link or from a nearby member — your old posts still exist on devices that synced them."
                ),
            ]
        ),

        UsingRiotGuideSection(
            id: "post-and-read",
            title: "Post and read updates",
            notes: [
                "Names on posts are self-claimed display names paired with a tag derived from the author's key. The tag proves the same key signed two posts; the name proves nothing by itself.",
            ],
            tasks: [
                UsingRiotGuideTask(
                    id: "post",
                    goal: "Post an update to your community",
                    steps: [
                        "Open Home and tap “Post an update”.",
                        "Write your update. You can review exactly what will be signed before it is saved.",
                        "Tap “Post an update” at the bottom to sign and save it.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "Saved and signed on this device. Exchange with someone nearby to share it. Posting never needs a connection — delivery happens later, when devices sync.",
                    recovery: "If posting fails, your draft is safe — review it and try again. Drafts also survive closing the composer and the app."
                ),
                UsingRiotGuideTask(
                    id: "read",
                    goal: "Read what is happening",
                    steps: [
                        "Open Home. The front page and open wire show your community's updates; editorial history is always public.",
                        "Open People to see who you have synced with and what they carry.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "Everything already on this device is readable with no connection.",
                    recovery: "An empty wire in a community you just joined means details have not arrived yet — its name and posts arrive on first sync with a member."
                ),
            ]
        ),

        UsingRiotGuideSection(
            id: "nearby",
            title: "Exchange nearby",
            notes: [
                "Discovery runs automatically nearby over Bluetooth or your local network. Nothing is added to your community without your confirmation: you preview what a peer offers first.",
            ],
            tasks: [
                UsingRiotGuideTask(
                    id: "exchange",
                    goal: "Exchange updates with a nearby device",
                    steps: [
                        "Both people open the Nearby screen in the same community.",
                        "Tap “Find nearby devices”.",
                        "When the other device appears, select it. The other person taps “Accept” (or “Decline”).",
                        "If new updates are offered, review the preview, then tap “Add … updates” to take them — or “Not now” to decline.",
                        "Tap “Stop” to end discovery when you are done.",
                    ],
                    connectivity: .needsPermission("local-network or Bluetooth permission"),
                    expectedResult: "“Synced” — you both carry the same updates, including posts either of you collected from others earlier.",
                    recovery: "“They are in a different space, so nothing was shared” means you are in different communities — switch to the same one and reconnect."
                ),
                UsingRiotGuideTask(
                    id: "nearby-permission",
                    goal: "Fix “Nearby needs permission”",
                    steps: [
                        "On the Nearby screen, read the permission card.",
                        "Tap “Open Settings” and allow local network access for Riot.",
                        "Return to Riot and tap “Find nearby devices” again.",
                    ],
                    connectivity: .needsPermission("a settings change"),
                    expectedResult: "Discovery starts. Everything else in Riot keeps working offline while permission is denied — only nearby exchange needs it.",
                    recovery: "If the device still does not appear, move the phones closer, keep both on the Nearby screen, and make sure both are on the same Wi-Fi network."
                ),
            ]
        ),

        UsingRiotGuideSection(
            id: "share",
            title: "Share a community",
            tasks: [
                UsingRiotGuideTask(
                    id: "share-link",
                    goal: "Invite someone with a link or QR code",
                    steps: [
                        "Open “Community settings” from the top of the screen.",
                        "Tap “Share this community”.",
                        "Send the share link, or let them scan the QR code on your screen.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "They can join with the link or QR code. Anyone with this link or QR code can follow the community — and can pass it onward; a public community's reference is itself public.",
                    recovery: "“Nothing to share yet” means this community's details have not reached this device — the link becomes available after its first sync. Check back after exchanging with a member."
                ),
            ]
        ),

        UsingRiotGuideSection(
            id: "tools",
            title: "Use community tools",
            notes: [
                "Tools are small apps the community carries with it. A tool runs only after the community's organizer turns it on — “Only the organizer of this community can turn a tool on here.”",
            ],
            tasks: [
                UsingRiotGuideTask(
                    id: "open-tool",
                    goal: "Open and use a tool",
                    steps: [
                        "Open Tools. Home also shows shortcuts to the first approved tools.",
                        "Tap a tool to open it. It opens inside the community, with the community header and tabs still visible.",
                        "If you are the organizer, review what a tool can access before approving it.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "Approved tools work entirely on this device and save their data into the community, so it syncs like everything else.",
                    recovery: "A tool that is unapproved, incomplete, or unavailable says so in place of opening — ask the community's organizer to review and approve it."
                ),
            ]
        ),

        UsingRiotGuideSection(
            id: "privacy",
            title: "Privacy and safety",
            notes: [
                "Public community content is plaintext by design. Alerts, mutual-aid requests, and public reporting are meant to circulate — Riot does not pretend they are secret.",
                "Pseudonymity is not anonymity. Separate communities use separate keys, but reused names, writing style, timing, and nearby radio presence can still correlate you.",
                "Nearby exchange reveals your device's presence and label to devices around you. A public web gateway you read through can log ordinary connection metadata.",
                "Riot cannot recall or erase copies other devices already accepted. Post as if every public update is permanent.",
                "Encrypted private groups are not available in this build — do not treat any current community as confidential.",
            ]
        ),

        UsingRiotGuideSection(
            id: "troubleshooting",
            title: "Troubleshooting",
            tasks: [
                UsingRiotGuideTask(
                    id: "ts-empty-community",
                    goal: "A joined community looks empty",
                    steps: [
                        "This is normal right after joining with a link or QR code — its name and posts arrive on first sync.",
                        "Exchange with a member on the Nearby screen to bring its content over.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "After the first exchange with a member, the community's name, front page, and wire fill in.",
                    recovery: "If it stays empty after syncing, the peer may be in a different community — check the community name at the top on both devices."
                ),
                UsingRiotGuideTask(
                    id: "ts-no-device",
                    goal: "No nearby device appears",
                    steps: [
                        "Keep both devices on the Nearby screen with the screen on.",
                        "Tap “Find nearby devices” on both.",
                        "Move the devices closer, and put both on the same Wi-Fi network.",
                        "If a permission card shows, tap “Open Settings” and allow local network access.",
                    ],
                    connectivity: .needsPermission("local-network or Bluetooth permission"),
                    expectedResult: "The other device appears in “Nearby devices”.",
                    recovery: "“The connection failed — try again” is safe to retry: tap “Find nearby devices” again on both devices."
                ),
                UsingRiotGuideTask(
                    id: "ts-recovery",
                    goal: "A community “Needs recovery before it can open.”",
                    steps: [
                        "In “Your communities”, tap “Retry” on the affected community.",
                        "If it cannot recover, the rest of your communities keep working — the affected data is quarantined, never deleted.",
                    ],
                    connectivity: .worksOffline,
                    expectedResult: "The community opens again, or stays safely quarantined while everything else works.",
                    recovery: "You can rejoin the same community with its link; quarantine keeps the old data aside instead of destroying it."
                ),
            ]
        ),

        UsingRiotGuideSection(
            id: "platforms",
            title: "Platform notes",
            notes: [
                "iPhone and Mac share the same four screens — Home, Tools, People, Nearby — and the same flows. On a Mac they are the sidebar, and Command-1 through Command-4 select them.",
                "Android organizes the same ideas with different labels (Spaces, App directory, Compose & sign, Connection); its own build documents them.",
                "This guide is stored inside the app and never fetches anything — it is complete and readable with no connection.",
            ]
        ),

        UsingRiotGuideSection(
            id: "not-yet",
            title: "What is not available yet",
            notes: [
                "Sync between Riot devices over the internet: today devices exchange nearby, and the public web view is rendered from exports — a browser reader trusts the gateway they chose. Internet sync between Riot devices is being built.",
                "Encrypted private groups: planned separately; no current community is confidential.",
                "Recalling or deleting every copy of a public post: impossible once others carry it.",
                "Production guarantees: Riot is a prototype — no audited security, guaranteed availability, or verified two-iPhone Bluetooth exchange; the tested nearby path is the local network.",
            ]
        ),
    ]
}

// MARK: - View

/// The manual, rendered natively: a contents list that pushes one section at a
/// time, so a person under pressure picks a task instead of scrolling a wall of
/// text. Everything is bundled — this view performs no network access.
public struct UsingRiotGuideView: View {
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    public init(onClose: @escaping () -> Void) {
        self.onClose = onClose
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Checked against this build · \(UsingRiotGuide.checkedDate)")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textCase(.uppercase)
                        .tracking(1)
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    RiotCard {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(UsingRiotGuide.sections) { section in
                                NavigationLink(value: section.id) {
                                    HStack {
                                        Text(section.title)
                                            .font(.riot(.body, size: 17, relativeTo: .body))
                                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                                            .multilineTextAlignment(.leading)
                                        Spacer()
                                        Image(systemName: "chevron.right")
                                            .font(.system(size: 12, weight: .semibold))
                                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                                    }
                                    .frame(minHeight: 44)
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("guide-section-\(section.id)")
                            }
                        }
                    }
                }
                .padding(20)
            }
            .riotHeader(eyebrow: "Help", UsingRiotGuide.entryLabel)
            .navigationDestination(for: String.self) { id in
                if let section = UsingRiotGuide.sections.first(where: { $0.id == id }) {
                    UsingRiotGuideSectionView(section: section)
                }
            }
            .safeAreaInset(edge: .bottom) {
                Button("Done", action: onClose)
                    .buttonStyle(.riotPrimary)
                    .accessibilityIdentifier("guide-done")
                    .padding(20)
            }
        }
    }
}

/// One section: orientation notes first, then each task as a card with its
/// goal, connectivity stamp, numbered steps, expected result, and recovery.
struct UsingRiotGuideSectionView: View {
    let section: UsingRiotGuideSection
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if !section.notes.isEmpty {
                    RiotCard {
                        VStack(alignment: .leading, spacing: 10) {
                            ForEach(section.notes, id: \.self) { note in
                                Text(note)
                                    .font(.riot(.body, size: 15, relativeTo: .callout))
                                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                            }
                        }
                    }
                }
                ForEach(section.tasks) { task in
                    taskCard(task)
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: UsingRiotGuide.entryLabel, section.title)
    }

    private func taskCard(_ task: UsingRiotGuideTask) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                Text(task.goal)
                    .font(.riot(.body, size: 17, relativeTo: .headline))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityAddTraits(.isHeader)
                RiotBadge(task.connectivity.label, stamped: true)
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(task.steps.enumerated()), id: \.offset) { index, step in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text("\(index + 1).")
                                .font(.riot(.mono, size: 14, relativeTo: .callout))
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            Text(step)
                                .font(.riot(.body, size: 15, relativeTo: .callout))
                                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        }
                    }
                }
                labeled("Result", task.expectedResult)
                labeled("If it goes wrong", task.recovery)
            }
        }
        .accessibilityIdentifier("guide-task-\(task.id)")
    }

    private func labeled(_ label: String, _ text: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.riot(.mono, size: 12, relativeTo: .caption))
                .textCase(.uppercase)
                .tracking(1)
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            Text(text)
                .font(.riot(.body, size: 15, relativeTo: .callout))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
    }
}
