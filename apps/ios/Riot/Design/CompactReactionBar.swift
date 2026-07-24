import Foundation
import SwiftUI

public extension ReactionKind {
    var glyph: String {
        switch self {
        case .support: "♥"
        case .solidarity: "✊︎"
        case .important: "!"
        case .grief: "◌"
        }
    }
}

public enum ReactionCountFormatter {
    public static func string(_ count: Int) -> String {
        String(max(0, min(count, 999))) + (count > 999 ? "+" : "")
    }
}

public enum CompactReactionScale: Sendable {
    case normal
    case accessibility3
}

public struct CompactReactionMetricPresentation: Equatable, Sendable {
    public let visualWidth: Double
    public let visualHeight: Double
    public let targetHeight: Double
    public let glyphSlotWidth: Double
    public let statusSlotWidth: Double
}

public enum CompactReactionMetrics {
    public static let minimumTarget = 44.0
    public static let normalVisualWidth = 72.0
    public static let normalVisualHeight = 32.0
    public static let spacing = 8.0
    public static let gridRowSpacing = 8.0
    public static let contentSpacing = 6.0
    public static let glyphPointSize = 15.0
    public static let countPointSize = 13.0
    public static let singleRowBreakpoint =
        normalVisualWidth * 4 + spacing * 3

    public static func columns(for proposedWidth: Double) -> Int {
        proposedWidth >= singleRowBreakpoint ? 4 : 2
    }

    public static func presentation(
        for scale: CompactReactionScale
    ) -> CompactReactionMetricPresentation {
        let height = switch scale {
        case .normal: normalVisualHeight
        case .accessibility3: 46.0
        }
        return CompactReactionMetricPresentation(
            visualWidth: normalVisualWidth,
            visualHeight: height,
            targetHeight: max(minimumTarget, height),
            glyphSlotWidth: 18,
            statusSlotWidth: 12)
    }
}

public enum ReactionLegendCopy {
    public static let text =
        "Reactions: ♥ Support · ✊︎ Solidarity · ! Important · ◌ Grief."
}

public enum ReactionThemeRole: Equatable, Sendable {
    case card
    case paper2
    case ink
    case inkSoft
    case pink
    case onReactionAccent
    case danger
}

public struct ReactionControlStylePresentation: Equatable, Sendable {
    public let fill: ReactionThemeRole
    public let foreground: ReactionThemeRole
    public let outline: ReactionThemeRole
    public let outlineWidth: Double
    public let opacity: Double
    public let scale: Double
    public let showsSpinner: Bool
    public let showsCheck: Bool
}

public struct ReactionControlContentPresentation: Equatable, Sendable {
    public let showsGlyph: Bool
    public let showsSpinner: Bool
    public let showsCheck: Bool
}

public struct ReactionControlVisualState: Equatable, Sendable {
    public let selected: Bool
    public let pending: Bool
    public let failed: Bool
    public let authorityDisabled: Bool
    public let hovered: Bool
    public let pressed: Bool

    public init(
        selected: Bool,
        pending: Bool,
        failed: Bool,
        authorityDisabled: Bool,
        hovered: Bool,
        pressed: Bool
    ) {
        self.selected = selected
        self.pending = pending
        self.failed = failed
        self.authorityDisabled = authorityDisabled
        self.hovered = hovered
        self.pressed = pressed
    }

    public var presentation: ReactionControlStylePresentation {
        var fill: ReactionThemeRole = selected ? .pink : .card
        let foreground: ReactionThemeRole = selected ? .onReactionAccent : .ink
        var outline: ReactionThemeRole = selected ? .ink : .inkSoft
        var outlineWidth = selected ? 2.0 : 1.0

        if failed && !pending && !authorityDisabled {
            outline = .danger
            outlineWidth = 2
        } else if hovered && !selected && !pending && !authorityDisabled {
            fill = .paper2
            outline = .ink
            outlineWidth = 2
        }

        let appliesPress = pressed && !pending && !authorityDisabled
        return ReactionControlStylePresentation(
            fill: fill,
            foreground: foreground,
            outline: outline,
            outlineWidth: outlineWidth,
            opacity: authorityDisabled ? 0.5 : appliesPress ? 0.88 : 1,
            scale: appliesPress ? 0.96 : 1,
            showsSpinner: pending && !authorityDisabled,
            showsCheck: selected)
    }
}

public struct ReactionControlPresentation: Equatable, Identifiable, Sendable {
    public let kind: ReactionKind
    public let count: Int
    public let selected: Bool
    public let pending: Bool
    public let failed: Bool
    public let disabled: Bool
    public let authorityDisabled: Bool

    public init(
        kind: ReactionKind,
        count: Int,
        selected: Bool,
        pending: Bool,
        failed: Bool,
        disabled: Bool = false,
        authorityDisabled: Bool = false
    ) {
        self.kind = kind
        self.count = max(0, count)
        self.selected = selected
        self.pending = pending
        self.failed = failed
        self.disabled = disabled
        self.authorityDisabled = authorityDisabled
    }

    public var id: ReactionKind { kind }
    public var countText: String { ReactionCountFormatter.string(count) }
    public var stableIdentifier: String { "reaction-\(kind.rawValue)" }
    public var accessibilityLabel: String { kind.label }
    public var help: String { kind.label }
    public var reservedWidth: Double { CompactReactionMetrics.normalVisualWidth }

    public var accessibilityValue: String {
        let noun = count == 1 ? "reaction" : "reactions"
        let tally = count == 0 ? "No reactions" : "\(countText) \(noun)"
        if pending { return "\(tally), saving" }
        if failed { return "\(tally), save failed" }
        if selected { return "\(tally), selected" }
        return "\(tally), not selected"
    }

    public var accessibilityHint: String {
        selected
            ? "Removes your reaction"
            : "Adds your reaction"
    }

    public func visualState(
        hovered: Bool,
        pressed: Bool
    ) -> ReactionControlVisualState {
        ReactionControlVisualState(
            selected: selected,
            pending: pending,
            failed: failed,
            authorityDisabled: authorityDisabled,
            hovered: hovered,
            pressed: pressed)
    }

    public var contentPresentation: ReactionControlContentPresentation {
        let resolved = visualState(hovered: false, pressed: false).presentation
        return ReactionControlContentPresentation(
            showsGlyph: !resolved.showsSpinner,
            showsSpinner: resolved.showsSpinner,
            showsCheck: resolved.showsCheck)
    }
}

public struct ReactionLegendStore {
    public static let key = "riot.reactionLegendDismissed.v1"

    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public var shouldShow: Bool {
        !defaults.bool(forKey: Self.key)
    }

    public func dismiss() {
        defaults.set(true, forKey: Self.key)
    }
}

public struct ReactionFailureOrder {
    private var nextSequence: UInt64 = 0
    private var sequenceByKey: [ReactionKey: UInt64] = [:]

    public init() {}

    public mutating func record(_ key: ReactionKey) {
        nextSequence &+= 1
        sequenceByKey[key] = nextSequence
    }

    public mutating func remove(_ key: ReactionKey) {
        sequenceByKey.removeValue(forKey: key)
    }

    public mutating func retain(_ keys: Set<ReactionKey>) {
        sequenceByKey = sequenceByKey.filter { keys.contains($0.key) }
    }

    public func latestKey(forPostID postID: String) -> ReactionKey? {
        latestKey(forPostID: postID, among: Set(sequenceByKey.keys))
    }

    public func latestKey(
        forPostID postID: String,
        among candidates: Set<ReactionKey>
    ) -> ReactionKey? {
        sequenceByKey
            .filter { $0.key.postID == postID && candidates.contains($0.key) }
            .max { $0.value < $1.value }?
            .key
    }
}

public enum ReactionFailureSelector {
    public static func latestVisible(
        failures: [ReactionKey: ReactionFailurePresentation],
        order: ReactionFailureOrder,
        postID: String,
        surface: ReactionSurface
    ) -> ReactionFailurePresentation? {
        let scoped = failures.filter {
            $0.key.postID == postID && $0.value.surface == surface
        }
        let committed = Set(scoped.compactMap { key, value in
            value.kind == .committedNeedsRefresh ? key : nil
        })
        let retryable = Set(scoped.compactMap { key, value in
            value.kind == .rejected && value.failure.kind == .retryablePersistence
                ? key
                : nil
        })
        let candidates = !committed.isEmpty
            ? committed
            : !retryable.isEmpty ? retryable : Set(scoped.keys)
        guard let key = order.latestKey(forPostID: postID, among: candidates) else {
            return nil
        }
        return failures[key]
    }
}

public struct ReactionAnnouncementCursor {
    private var lastSequence: UInt64 = 0

    public init() {}

    public mutating func shouldAnnounce(sequence: UInt64) -> Bool {
        guard sequence > lastSequence else { return false }
        lastSequence = sequence
        return true
    }
}

public struct ReactionAnnouncementConsumption {
    private var cursors: [ReactionSurface: ReactionAnnouncementCursor] = [:]

    public init() {}

    public mutating func consumeLatest(
        in announcements: [ReactionAnnouncement],
        surface: ReactionSurface
    ) -> ReactionAnnouncement? {
        guard let announcement = announcements.last(where: { $0.surface == surface }) else {
            return nil
        }
        var cursor = cursors[surface] ?? ReactionAnnouncementCursor()
        guard cursor.shouldAnnounce(sequence: announcement.sequence) else { return nil }
        cursors[surface] = cursor
        return announcement
    }
}

public struct CompactReactionBar: View {
    public let rowToken: String
    public let controls: [ReactionControlPresentation]
    public let failureMessage: String?
    public let showsLegend: Bool
    public let onToggle: (ReactionKind) -> Void
    public let onDismissLegend: () -> Void

    public init(
        rowToken: String,
        controls: [ReactionControlPresentation],
        failureMessage: String?,
        showsLegend: Bool,
        onToggle: @escaping (ReactionKind) -> Void,
        onDismissLegend: @escaping () -> Void
    ) {
        self.rowToken = rowToken
        self.controls = controls
        self.failureMessage = failureMessage
        self.showsLegend = showsLegend
        self.onToggle = onToggle
        self.onDismissLegend = onDismissLegend
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if showsLegend {
                reactionLegend
            }
            ViewThatFits(in: .horizontal) {
                HStack(spacing: CompactReactionMetrics.spacing) {
                    controlViews
                }
                .fixedSize(horizontal: true, vertical: false)

                LazyVGrid(
                    columns: Array(
                        repeating: GridItem(
                            .fixed(CompactReactionMetrics.normalVisualWidth),
                            spacing: CompactReactionMetrics.spacing),
                        count: 2),
                    alignment: .leading,
                    spacing: CompactReactionMetrics.gridRowSpacing
                ) {
                    controlViews
                }
            }
            if let failureMessage {
                Text(failureMessage)
                    .font(.riot(.body, size: 12, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.danger(for: colorScheme))
                    .accessibilityIdentifier("reaction-error-\(rowToken)")
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Reactions")
        .accessibilityIdentifier("reaction-bar-\(rowToken)")
    }

    @Environment(\.colorScheme) private var colorScheme

    @ViewBuilder
    private var controlViews: some View {
        ForEach(controls) { control in
            CompactReactionControl(
                presentation: control,
                rowToken: rowToken,
                action: { onToggle(control.kind) })
        }
    }

    private var reactionLegend: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(ReactionLegendCopy.text)
                .font(.riot(.mono, size: 10, relativeTo: .caption2))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                .fixedSize(horizontal: false, vertical: true)
            Button("Got it", action: onDismissLegend)
                .buttonStyle(.plain)
                .font(.riot(.body, size: 12, relativeTo: .caption))
                .foregroundStyle(RiotTheme.accent(for: colorScheme))
                .frame(minHeight: CompactReactionMetrics.minimumTarget)
                .accessibilityIdentifier("reaction-legend-dismiss")
        }
        .accessibilityIdentifier("reaction-legend")
    }
}

private struct CompactReactionControl: View {
    let presentation: ReactionControlPresentation
    let rowToken: String
    let action: () -> Void

    @Environment(\.colorScheme) private var colorScheme
    @ScaledMetric(relativeTo: .caption) private var visualHeight =
        CompactReactionMetrics.normalVisualHeight
    @ScaledMetric(relativeTo: .caption) private var glyphSize =
        CompactReactionMetrics.glyphPointSize
    @ScaledMetric(relativeTo: .caption2) private var countSize =
        CompactReactionMetrics.countPointSize
    @State private var hovered = false
    @FocusState private var focused: Bool

    var body: some View {
        let contentPresentation = presentation.contentPresentation
        Button(action: action) {
            HStack(spacing: CompactReactionMetrics.contentSpacing) {
                ZStack {
                    Text(presentation.kind.glyph)
                        .font(.system(size: glyphSize, weight: .semibold))
                        .opacity(contentPresentation.showsGlyph ? 1 : 0)
                    ProgressView()
                        .controlSize(.mini)
                        .frame(width: 14, height: 14)
                        .tint(
                            presentation.selected
                                ? RiotTheme.onReactionAccent(for: colorScheme)
                                : RiotTheme.ink(for: colorScheme))
                        .opacity(contentPresentation.showsSpinner ? 1 : 0)
                }
                .frame(width: 15)

                Text(presentation.countText)
                    .font(.riot(.mono, size: countSize, relativeTo: .caption2))
                    .monospacedDigit()
                    .lineLimit(1)
                    .minimumScaleFactor(0.72)
                    .frame(width: 26, alignment: .trailing)

                Image(systemName: "checkmark")
                    .font(.system(size: 9, weight: .bold))
                    .opacity(contentPresentation.showsCheck ? 1 : 0)
                    .frame(width: 9)
            }
            .frame(
                width: CompactReactionMetrics.normalVisualWidth,
                height: max(CompactReactionMetrics.normalVisualHeight, visualHeight))
            .contentShape(Capsule())
        }
        .buttonStyle(CompactReactionButtonStyle(
            state: presentation.visualState(
                hovered: hovered,
                pressed: false),
            hovered: hovered,
            colorScheme: colorScheme))
        .frame(width: CompactReactionMetrics.normalVisualWidth)
        .frame(minHeight: max(CompactReactionMetrics.minimumTarget, visualHeight))
        .contentShape(Rectangle())
        .disabled(presentation.disabled || presentation.pending)
        .focused($focused)
        .onHover { hovered = $0 }
        .help(presentation.help)
        .accessibilityLabel(presentation.accessibilityLabel)
        .accessibilityValue(presentation.accessibilityValue)
        .accessibilityHint(presentation.accessibilityHint)
        .accessibilityAddTraits(presentation.selected ? .isSelected : [])
        .accessibilityIdentifier(
            "\(presentation.stableIdentifier)-\(rowToken)")
#if os(iOS)
        .contextMenu {
            Text(presentation.kind.label)
                .disabled(true)
        }
#endif
    }
}

private struct CompactReactionButtonStyle: ButtonStyle {
    let state: ReactionControlVisualState
    let hovered: Bool
    let colorScheme: ColorScheme

    func makeBody(configuration: Configuration) -> some View {
        let visual = ReactionControlVisualState(
            selected: state.selected,
            pending: state.pending,
            failed: state.failed,
            authorityDisabled: state.authorityDisabled,
            hovered: hovered,
            pressed: configuration.isPressed).presentation
        configuration.label
            .foregroundStyle(color(visual.foreground))
            .background(color(visual.fill))
            .clipShape(Capsule())
            .overlay {
                Capsule()
                    .stroke(
                        color(visual.outline),
                        lineWidth: visual.outlineWidth)
            }
            .shadow(
                color: hovered && !state.pending && !state.authorityDisabled
                    ? RiotTheme.ink(for: colorScheme).opacity(0.08)
                    : .clear,
                radius: 2,
                y: 1)
            .scaleEffect(visual.scale)
            .opacity(visual.opacity)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }

    private func color(_ role: ReactionThemeRole) -> Color {
        switch role {
        case .card: RiotTheme.card(for: colorScheme)
        case .paper2: RiotTheme.paper2(for: colorScheme)
        case .ink: RiotTheme.ink(for: colorScheme)
        case .inkSoft: RiotTheme.inkSoft(for: colorScheme)
        case .pink: RiotTheme.pink(for: colorScheme)
        case .onReactionAccent: RiotTheme.onReactionAccent(for: colorScheme)
        case .danger: RiotTheme.danger(for: colorScheme)
        }
    }
}

public struct ReactionAccessibilityAnnouncementHost: View {
    public let announcements: [ReactionAnnouncement]
    public let consumeLatest: () -> String?

    public init(
        announcements: [ReactionAnnouncement],
        consumeLatest: @escaping () -> String?
    ) {
        self.announcements = announcements
        self.consumeLatest = consumeLatest
    }

    public var body: some View {
        Color.clear
            .frame(width: 0, height: 0)
            .accessibilityHidden(true)
            .onAppear(perform: announceLatest)
            .onChange(of: announcements) {
                announceLatest()
            }
    }

    private func announceLatest() {
        guard let message = consumeLatest() else { return }
        AccessibilityNotification.Announcement(message).post()
    }
}
