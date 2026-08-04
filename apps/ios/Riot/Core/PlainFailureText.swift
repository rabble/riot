import Foundation

/// Turns a refusal from the core into a sentence a person can act on.
///
/// The core's refusals are precise and they are *for the core*: `StalePreview`
/// tells a developer that the store moved under a preview handle; it tells the
/// person holding the phone nothing at all. Before this type, several call sites
/// did `errorMessage = String(describing: error)`, so the alert on the launch
/// screen — the one surface a stuck person has — read `Database` or
/// `CorruptDatabase(message: "…")`.
///
/// Two rules hold here:
///
/// 1. **Every message names a next step.** A refusal a person cannot act on is
///    the same as a crash, just quieter. Where the honest next step is "wait",
///    it says wait.
/// 2. **Nothing is claimed that the core does not guarantee.** These sentences
///    do not promise that state is unchanged unless the refusal happened before
///    the commit swap. Riot's honesty contract applies to error copy too.
///
/// The raw string is not thrown away — ``technical(for:)`` is what goes to the
/// unified log and the Diagnostics panel, where a developer looks for it.
public enum PlainFailureText {
    /// What a person is told when the refusal has no case of its own — a Swift
    /// error from the repository, the filesystem, or a decoder. It stays vague
    /// on purpose: the specific thing that broke is in the log, and guessing at
    /// a cause here would be worse than admitting there isn't one.
    public static let unknown = "Something stopped that from finishing. Try again."

    /// The sentence to show a person. Never the enum, on any path.
    public static func plain(for error: Error) -> String {
        guard let mobile = error as? MobileError else { return unknown }
        switch mobile {
        case .Internal:
            return "Something went wrong inside Riot. What you wrote is still here — try that again."
        case .SessionFailed:
            return "Riot stopped partway to keep what you already have safe. Close Riot and open it again."
        case .InvalidInput:
            return "That didn’t look like something Riot could use. Check what you typed and try again."
        case .DraftNotFound:
            return "That draft isn’t here any more. Write it again."
        case .ImportRejected:
            return "Riot couldn’t read what you were given. Ask whoever shared it to send it again."
        case .StoreFull:
            return "This community has no more room on this device. Leave one you no longer use to make space."
        case .SessionLimit:
            return "Riot has too much open at once. Close what you’re doing and try again."
        case .ObjectClosed:
            return "That screen is no longer open. Go back and start it again."
        case .PreviewConsumed:
            return "That was already handled once. Nothing changed this time."
        case .PlanConsumed:
            return "That step was already taken. Go back to see where things stand."
        case .StalePreview:
            return "Something arrived while you were looking. Try again to work from what’s here now."
        case .EntropyUnavailable:
            return "Riot couldn’t post that safely right now. Wait a moment and try again."
        case .ClockUnavailable:
            return "Riot couldn’t read this device’s clock, so it stopped rather than post with the wrong time. "
                + "Check the date and time in Settings."
        case .AppRejected:
            return "That tool couldn’t be added. Ask whoever shared it to send it again."
        case .NotSpaceOrganizer:
            return "Only this community’s organizer can do that."
        case .LegacyProfileCannotOrganize:
            return "This profile was made before communities had organizers, so it can’t organize one. "
                + "Start a new profile if you want to run your own community."
        case .Database:
            return "Riot couldn’t reach what’s saved on this device. Open Riot again; if it keeps happening, "
                + "use Start fresh."
        case .CommunityUnavailable:
            return "That community wouldn’t open. It’s still in your list, so you can try it again from there."
        }
    }

    /// The raw refusal, for the log and the Diagnostics panel — never for a
    /// person's alert.
    public static func technical(for error: Error) -> String {
        String(describing: error)
    }
}
