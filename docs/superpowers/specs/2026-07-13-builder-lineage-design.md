# Builder and Indymedia Lineage Design

## Goal

Make the person and lived history behind Riot visible on the marketing homepage,
and connect Riot's seizure-resistant design to documented government action
against Indymedia without overstating either the technology or the history.

## Placement

Extend the existing dark Lineage section with a bordered builder card rather
than adding a biography page. Add a `Builder` link to the desktop navigation and
credit `@rabble` again in the footer so the authorship is findable without
making it the homepage's primary user proposition.

## Copy and attribution

Credit `@rabble` with building Riot using the Willow libraries. Do not publish a
legal name, claim that `@rabble` implemented Willow, or imply sole authorship of
the Willow protocol specification. Summarize the relevant through-line:
protest.net, Indymedia's technology network, TXTMob, Odeo/Twitter, Planetary,
Nos, Divine, and Riot.

The builder card has two supporting columns:

1. `A long line of movement infrastructure` explains the practical history.
2. `Seizure resistance is not theoretical` distinguishes the German government's
   2017 shutdown and raids against Linksunten Indymedia from the German state
   interior ministers' June 2026 request to examine a complete ban of
   de.indymedia.org.

## Sources

Link the builder history to the Columbia Journalism Review's Indymedia history
and the Nos biography. Link the government-action claims to the supplied 2017
CrimethInc account and 2026 Heise report. The page may characterize these events
as evidence for resilient infrastructure, but must present dates and actions as
reported facts rather than claiming Riot would defeat every legal or technical
attack.

## Visual and responsive behavior

Use the existing paper/pink/blue visual language, typography, and sharp borders.
The builder card uses a two-column grid on desktop and one column below 760px.
No portrait, new font, remote image, background graphic, or JavaScript is added.

## Verification

The marketing contract pins the builder anchor, `@rabble` credit, Willow-library
dependency wording, both government-action dates, and all four source URLs. It
also rejects claims that `@rabble` implemented Willow. Source and public
homepage mirrors remain byte-identical. Visual QA covers desktop and mobile,
and release verification compares the live Worker bytes to the committed public
mirror.
