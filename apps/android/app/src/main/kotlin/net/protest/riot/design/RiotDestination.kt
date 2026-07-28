package net.protest.riot.design

import net.protest.riot.ConferenceSurface

/**
 * The four places this app has, matching the iOS shell exactly
 * (`RiotShellDestination`: Home, Tools, People, Nearby).
 *
 * WHY THE RESHAPE. Android had eight bottom tabs — Spaces, App directory,
 * Incident board, Newswire, Follow a site, Compose & sign, Import preview,
 * Connection. That is not a navigation bar, it is a menu of the app's internal
 * surfaces, and on a phone it could not even render its own labels. iOS long ago
 * moved to a community-first shell where the four tabs are PLACES a person goes
 * and everything else is an action taken inside one of them.
 *
 * NOTHING WAS DROPPED. Every one of the eight surfaces is still reachable; the
 * four that are not places became actions that push their existing screen with a
 * back affordance ([pushedFrom]). A capability that a person can no longer find
 * has been deleted whether or not the code still compiles, so the mapping below
 * is the reviewable part of this change.
 */
enum class RiotDestination(val label: String) {
    /** The wire, the alerts, and — until a community exists — starting one. */
    HOME("Home"),

    /** The tools this community carries, and the directory they come from. */
    TOOLS("Tools"),

    /** Who is on this wire, derived from the posts they signed. */
    PEOPLE("People"),

    /** Radios: pairing, sync, and carrying a bundle in by hand. */
    NEARBY("Nearby");

    companion object {
        val tabs = entries.toList()
    }
}

/**
 * The surfaces that are actions rather than places, and the tab each is reached
 * from. Kept as data so the mapping can be read — and tested — in one place
 * rather than inferred from button placement across three files.
 */
val ConferenceSurface.pushedFrom: RiotDestination?
    get() =
        when (this) {
            // Places, not pushes.
            ConferenceSurface.NEWSWIRE,
            ConferenceSurface.INCIDENT_BOARD,
            ConferenceSurface.SPACES,
            -> null
            ConferenceSurface.APP_DIRECTORY -> RiotDestination.TOOLS
            // Following a site is how a community carries someone else's
            // publication, so it sits with the other things it carries.
            ConferenceSurface.FOLLOW_SITES -> RiotDestination.TOOLS
            // Writing and signing is an act on the wire.
            ConferenceSurface.COMPOSE_AND_SIGN -> RiotDestination.HOME
            // A bundle arrives from a person, over a radio or a file — the same
            // place pairing lives.
            ConferenceSurface.IMPORT_PREVIEW -> RiotDestination.NEARBY
            ConferenceSurface.CONNECTION -> RiotDestination.NEARBY
        }
