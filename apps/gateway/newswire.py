#!/usr/bin/env python3
"""Two-column newswire render profile: editorial features (E) + open wire (W).

This is the "mockup made real" as a render function. It takes a NewswireView
— the shape the composite-site model produces (E editorial + W open-wire) — and
emits a self-contained, CSP-fenced page in the reach-layer style
(`docs/superpowers/specs/2026-07-16-web-viewer-reach-layer-design.md`).

It is deliberately decoupled from the hash-locked conference gateway: it renders
a supplied view, and `sample_view()` returns clearly-flagged DEMO data so the
layout can be seen before real signed E/W content exists (composite Unit 1/2).
Same fences as the rest of the reach layer: inline CSS, no external anything,
`default-src 'none'`, deep link to the app for the verified copy.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from html import escape

from riot_gateway import _sri_sha256

# Unix seconds at the J2000 epoch (2000-01-01T12:00:00Z). Willow timestamps are
# TAI/J2000 microseconds; the export carries them raw and the gateway formats.
# TAI leads UTC by ~64s at J2000 — negligible for a minute-resolution display.
_J2000_UNIX_SECONDS = 946_728_000


@dataclass(frozen=True)
class EditorialEntry:
    """A signed editorial article (namespace E, owned, verified)."""

    category: str
    title: str
    summary: str
    author: str
    time: str
    verified: bool = True
    ai_assisted: bool = False
    # When set, the headline links to this post's own page; empty for the demo.
    entry_id: str = ""


@dataclass(frozen=True)
class WirePost:
    """An open-published post (namespace W, communal, unverified)."""

    time: str
    handle: str
    body: str
    ai_assisted: bool = False
    entry_id: str = ""


@dataclass(frozen=True)
class PostView:
    """A single post's own page, resolved from the /2 export."""

    entry_id: str
    title: str
    body: str
    author: str
    signer: str
    verified: bool
    ai_assisted: bool
    namespace: str
    space_name: str


@dataclass(frozen=True)
class AuthorPostRef:
    entry_id: str
    title: str
    verified: bool


@dataclass(frozen=True)
class AuthorView:
    """A contributor's page: who they are + what they've published."""

    author_id: str
    display_name: str
    is_organizer: bool
    contribution_count: int
    posts: tuple[AuthorPostRef, ...]
    namespace: str
    space_name: str


@dataclass(frozen=True)
class NewswireView:
    name: str
    tagline: str
    namespace: str
    categories: tuple[str, ...]
    editorial: tuple[EditorialEntry, ...]
    wire: tuple[WirePost, ...]
    mirror_note: str
    sample: bool = False


NEWSPRINT_CSS = """
:root {
  --paper: #f4f1e9; --panel: #eae5d7; --ink: #17150f; --muted: #6c675a;
  --line: rgba(23,21,15,0.16); --red: #cc2222; --wire: #9a5a12; --verify: #2f6b43;
  color-scheme: light dark;
}
@media (prefers-color-scheme: dark) {
  :root { --paper:#14140f; --panel:#1c1b15; --ink:#ece7d7; --muted:#9a9484; --line:rgba(236,231,215,0.15); --red:#ff5a4d; --wire:#d69a45; --verify:#6fb185; }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--paper); color: var(--ink); font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; line-height: 1.5; }
a { color: inherit; }
.masthead { border-bottom: 3px solid var(--ink); padding: 0.9rem 1.25rem 0; max-width: 78rem; margin: 0 auto; }
.masthead__top { display: flex; align-items: baseline; gap: 0.9rem; flex-wrap: wrap; }
.wordmark { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: clamp(2rem,6vw,3.2rem); letter-spacing: -0.03em; line-height: 0.9; margin: 0; }
.masthead__tag { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.18em; color: var(--muted); }
.masthead__meta { margin-left: auto; text-align: right; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.66rem; color: var(--muted); line-height: 1.5; }
.cats { display: flex; gap: 0 1.1rem; flex-wrap: wrap; margin: 0.85rem 0 0; padding: 0.5rem 0; border-top: 1px solid var(--line); list-style: none; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.74rem; letter-spacing: 0.05em; text-transform: uppercase; }
.cats a { text-decoration: none; color: var(--muted); padding-bottom: 2px; border-bottom: 2px solid transparent; }
.cats a.on { color: var(--ink); border-bottom-color: var(--red); }
.shell { max-width: 78rem; margin: 0 auto; padding: 1.5rem 1.25rem 4rem; display: grid; grid-template-columns: minmax(0,1fr) 21rem; gap: 2.5rem; }
.section-label { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.14em; color: var(--muted); margin: 0 0 1.1rem; display: flex; align-items: center; gap: 0.6rem; }
.section-label::after { content: ""; flex: 1; height: 1px; background: var(--line); }
.kicker { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.7rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.1em; color: var(--wire); }
.lede { border-bottom: 1px solid var(--line); padding-bottom: 1.75rem; margin-bottom: 1.75rem; }
.lede .kicker { color: var(--red); }
.lede h2 { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: clamp(1.9rem,4.6vw,2.9rem); line-height: 1.03; letter-spacing: -0.02em; margin: 0.5rem 0 0.6rem; }
.lede p { margin: 0 0 0.9rem; font-size: 1.08rem; max-width: 42rem; }
.byline { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; color: var(--muted); display: flex; gap: 0.55rem; flex-wrap: wrap; align-items: center; }
.byline .who { color: var(--ink); }
.verified { color: var(--verify); font-weight: 600; }
.verified::before { content: "\\2713 "; }
.ai { color: var(--muted); font-style: italic; }
.stories { display: grid; grid-template-columns: 1fr 1fr; gap: 1.6rem 2rem; }
.story { border-top: 2px solid var(--ink); padding-top: 0.7rem; }
.story h3 { font-family: Georgia, "Times New Roman", serif; font-weight: 700; font-size: 1.3rem; line-height: 1.15; margin: 0.4rem 0 0.5rem; }
.story p { margin: 0 0 0.7rem; font-size: 0.96rem; }
.wire { align-self: start; position: sticky; top: 1rem; background: var(--panel); border: 1px solid var(--line); border-top: 3px solid var(--wire); }
.wire__head { padding: 0.7rem 0.9rem; border-bottom: 1px dashed var(--line); font-family: ui-monospace, Menlo, Consolas, monospace; }
.wire__head strong { display: block; font-size: 0.82rem; letter-spacing: 0.14em; text-transform: uppercase; }
.wire__head span { font-size: 0.68rem; color: var(--muted); }
.wire__note { padding: 0.5rem 0.9rem; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.65rem; line-height: 1.5; color: var(--wire); border-bottom: 1px solid var(--line); }
.wire ol { list-style: none; margin: 0; padding: 0; }
.post { padding: 0.6rem 0.9rem; border-bottom: 1px solid var(--line); font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.78rem; line-height: 1.45; }
.post .t { color: var(--wire); }
.post .h { color: var(--ink); }
.post .body { display: block; margin-top: 0.15rem; }
.post .open { display: inline-block; margin-top: 0.3rem; font-size: 0.62rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--muted); border: 1px solid var(--line); padding: 0 0.3rem; border-radius: 2px; }
.wire__foot { padding: 0.7rem 0.9rem; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.68rem; }
.wire__foot a { color: var(--red); font-weight: 600; text-decoration: none; }
.foot { max-width: 78rem; margin: 0 auto; padding: 1.4rem 1.25rem 3rem; border-top: 1px solid var(--line); font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.68rem; color: var(--muted); line-height: 1.6; display: flex; gap: 0.6rem 1.5rem; flex-wrap: wrap; align-items: baseline; }
.foot a { color: var(--ink); }
.foot .sample { color: var(--red); }
@media (max-width: 780px) { .shell { grid-template-columns: 1fr; gap: 1.75rem; } .stories { grid-template-columns: 1fr; } .wire { position: static; } }
:focus-visible { outline: 2px solid var(--red); outline-offset: 2px; }
.headline-link { text-decoration: none; }
.headline-link:hover { text-decoration: underline; }
.article, .narrow { max-width: 44rem; margin: 0 auto; padding: 1.5rem 1.25rem 4rem; }
.back { display: inline-block; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; color: var(--muted); text-decoration: none; margin-bottom: 1rem; }
.article h1, .narrow h1 { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: clamp(1.7rem,4vw,2.5rem); line-height: 1.05; margin: 0.4rem 0 0.7rem; }
.article__body { font-size: 1.06rem; margin: 1rem 0 1.6rem; max-width: 42rem; }
.provenance { border-top: 1px solid var(--line); padding-top: 0.9rem; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; color: var(--muted); line-height: 1.9; }
.provenance b { color: var(--ink); font-weight: 600; }
.mono { word-break: break-all; }
.role { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.74rem; color: var(--muted); text-transform: uppercase; letter-spacing: 0.08em; }
.feed { list-style: none; margin: 1.2rem 0 0; padding: 0; }
.feed li { border-top: 1px solid var(--line); padding: 0.7rem 0; }
.feed a { text-decoration: none; }
.feed a:hover { text-decoration: underline; }
""".strip()


def _content_security_policy(css: str) -> str:
    return (
        "default-src 'none'; "
        f"style-src 'sha256-{_sri_sha256(css)}'; "
        "script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'"
    )


def _cats(categories: tuple[str, ...]) -> str:
    items = []
    for index, cat in enumerate(categories):
        cls = ' class="on"' if index == 0 else ""
        items.append(f'<li><a{cls} href="#">{escape(cat)}</a></li>')
    return "".join(items)


def _ai_marker(ai_assisted: bool) -> str:
    return '<span>·</span><span class="ai">AI-assisted</span>' if ai_assisted else ""


def _headline(text: str, entry_id: str) -> str:
    """The headline, linked to its own post page when the entry id is known."""
    label = escape(text)
    if entry_id:
        return f'<a class="headline-link" href="/post/{escape(entry_id)}/">{label}</a>'
    return label


def _lede(entry: EditorialEntry) -> str:
    verify = '<span class="verified">verified editorial</span>' if entry.verified else ""
    return f"""<article class="lede">
  <span class="kicker">{escape(entry.category)}</span>
  <h2>{_headline(entry.title, entry.entry_id)}</h2>
  <p>{escape(entry.summary)}</p>
  <div class="byline"><span class="who">{escape(entry.author)}</span><span>·</span><span>{escape(entry.time)}</span>{_ai_marker(entry.ai_assisted)}<span>·</span>{verify}</div>
</article>"""


def _story(entry: EditorialEntry) -> str:
    verify = '<span class="verified">verified</span>' if entry.verified else ""
    return f"""<article class="story">
  <span class="kicker">{escape(entry.category)}</span>
  <h3>{_headline(entry.title, entry.entry_id)}</h3>
  <p>{escape(entry.summary)}</p>
  <div class="byline"><span class="who">{escape(entry.author)}</span><span>·</span><span>{escape(entry.time)}</span>{_ai_marker(entry.ai_assisted)}<span>·</span>{verify}</div>
</article>"""


def _post(post: WirePost) -> str:
    ai = ' <span class="ai">AI-assisted</span>' if post.ai_assisted else ""
    body = _headline(post.body, post.entry_id)
    return (
        f'<li class="post"><span class="t">{escape(post.time)}</span> '
        f'<span class="h">{escape(post.handle)}</span>'
        f'<span class="body">{body}</span>'
        f'<span class="open">open · unverified</span>{ai}</li>'
    )


def render_newswire(view: NewswireView, css: str = NEWSPRINT_CSS) -> str:
    csp = _content_security_policy(css)
    editorial = view.editorial
    lede = _lede(editorial[0]) if editorial else ""
    stories = "".join(_story(entry) for entry in editorial[1:])
    posts = "".join(_post(post) for post in view.wire)
    namespace_uri = f"riot://open?namespace={view.namespace}"
    sample_note = '<span class="sample">demo · sample content, not signed</span>' if view.sample else ""
    return f"""<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="{csp}"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{escape(view.name)} · Newswire</title><style>{css}</style></head>
<body>
<header class="masthead">
  <div class="masthead__top">
    <h1 class="wordmark">{escape(view.name)}</h1>
    <span class="masthead__tag">{escape(view.tagline)}</span>
    <div class="masthead__meta">{escape(view.mirror_note)}</div>
  </div>
  <ul class="cats">{_cats(view.categories)}</ul>
</header>
<main class="shell">
  <section aria-label="Editorial features">
    <p class="section-label">Editorial · signed by the collective</p>
    {lede}
    <div class="stories">{stories}</div>
  </section>
  <aside class="wire" aria-label="Open newswire">
    <div class="wire__head"><strong>Open Newswire</strong><span>open publishing · anyone can post</span></div>
    <p class="wire__note">Unverified. Posted directly by readers over the p2p network. Read with care.</p>
    <ol>{posts}</ol>
    <p class="wire__foot"><a href="{namespace_uri}">+ Publish to the wire →</a></p>
  </aside>
</main>
<footer class="foot">
  <span>Served from a mirror · content signed by the collective, not this host</span>
  <span>Verified copy? <a href="{namespace_uri}">Open in Riot →</a></span>
  {sample_note}
</footer>
</body>
</html>"""


# Static, non-functional category chrome (no topic field on the /2 export yet).
_DEFAULT_CATEGORIES = ("Latest", "Housing", "Labor", "Surveillance", "Ecology", "Repression")


def _format_j2000(tai_j2000_micros: int) -> str:
    """A Willow TAI/J2000-microsecond timestamp as a minute-resolution UTC string."""
    unix = tai_j2000_micros // 1_000_000 + _J2000_UNIX_SECONDS
    return datetime.fromtimestamp(unix, tz=timezone.utc).strftime("%Y-%m-%d %H:%M UTC")


def newswire_view_from_export(export: dict) -> NewswireView:
    """Build the two-column E/W view from a `riot-public-gateway-export/2` newswire
    export. Featured entries become editorial features; the rest are open wire.
    Bylines are the author's signed display name only (never the key tag); an
    author with no card is a nameless open contributor. Entries whose signature
    did not re-verify are dropped — the gateway never renders unverifiable bytes.
    """
    names = {c["author_id"]: c for c in export.get("contributors", [])}
    editorial: list[EditorialEntry] = []
    wire: list[WirePost] = []
    seen: set[str] = set()
    for entry in export.get("entries", []):
        if entry.get("verification_status") == "signature_invalid":
            continue
        entry_id = entry.get("entry_id", "")
        if entry_id in seen:
            continue
        seen.add(entry_id)
        display_name = names.get(entry.get("signer"), {}).get("display_name") or "Open contributor"
        when = _format_j2000(entry.get("tai_j2000_micros", 0))
        ai_assisted = bool(entry.get("ai_assisted"))
        if entry.get("featured"):
            editorial.append(
                EditorialEntry(
                    category="Dispatch",
                    title=entry.get("title", ""),
                    summary=entry.get("body", ""),
                    author=display_name,
                    time=when,
                    verified=bool(entry.get("editorially_verified")),
                    ai_assisted=ai_assisted,
                    entry_id=entry_id,
                )
            )
        else:
            wire.append(
                WirePost(
                    time=when,
                    handle=display_name,
                    body=entry.get("body", ""),
                    ai_assisted=ai_assisted,
                    entry_id=entry_id,
                )
            )
    return NewswireView(
        name=export.get("title", "RIOT"),
        tagline="Independent Newswire · publish anywhere",
        namespace=export.get("namespace", ""),
        categories=_DEFAULT_CATEGORIES,
        editorial=tuple(editorial),
        wire=tuple(wire),
        mirror_note=f"updated {export.get('generated_at', '')}",
        sample=False,
    )


def all_post_ids(export: dict) -> list[str]:
    """Every renderable post's entry id (signature_invalid dropped) — the set of
    per-post pages `build.py` emits."""
    return [
        entry["entry_id"]
        for entry in export.get("entries", [])
        if entry.get("verification_status") != "signature_invalid" and entry.get("entry_id")
    ]


def post_view_from_export(export: dict, entry_id: str) -> PostView | None:
    """The single-post page model for `entry_id`, or None if it is absent or did
    not re-verify."""
    names = {c["author_id"]: c for c in export.get("contributors", [])}
    for entry in export.get("entries", []):
        if entry.get("entry_id") != entry_id:
            continue
        if entry.get("verification_status") == "signature_invalid":
            return None
        display_name = names.get(entry.get("signer"), {}).get("display_name") or "Open contributor"
        return PostView(
            entry_id=entry_id,
            title=entry.get("title", ""),
            body=entry.get("body", ""),
            author=display_name,
            signer=entry.get("signer", ""),
            verified=bool(entry.get("editorially_verified")),
            ai_assisted=bool(entry.get("ai_assisted")),
            namespace=export.get("namespace", ""),
            space_name=export.get("title", "RIOT"),
        )
    return None


def author_view_from_export(export: dict, author_id: str) -> AuthorView | None:
    """The contributor page for `author_id`. Only authors that carry a display-name
    card (i.e. appear in `contributors[]`) get a page — a nameless communal poster
    does not."""
    card = {c["author_id"]: c for c in export.get("contributors", [])}.get(author_id)
    if card is None:
        return None
    posts = tuple(
        AuthorPostRef(
            entry_id=entry["entry_id"],
            title=entry.get("title") or entry.get("body") or "",
            verified=bool(entry.get("editorially_verified")),
        )
        for entry in export.get("entries", [])
        if entry.get("signer") == author_id
        and entry.get("verification_status") != "signature_invalid"
        and entry.get("entry_id")
    )
    return AuthorView(
        author_id=author_id,
        display_name=card.get("display_name", author_id),
        is_organizer=bool(card.get("is_organizer")),
        contribution_count=int(card.get("contribution_count", 0)),
        posts=posts,
        namespace=export.get("namespace", ""),
        space_name=export.get("title", "RIOT"),
    )


def _doc(title: str, css: str, body: str) -> str:
    """A self-contained, CSP-fenced page wrapper shared by the post + author pages."""
    csp = _content_security_policy(css)
    return f"""<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="{csp}"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{escape(title)}</title><style>{css}</style></head>
<body>
{body}
</body>
</html>"""


def _page_masthead(space_name: str) -> str:
    return f"""<header class="masthead">
  <div class="masthead__top">
    <h1 class="wordmark"><a href="/">{escape(space_name)}</a></h1>
    <span class="masthead__tag">projected from signed records</span>
  </div>
</header>"""


def _page_footer(namespace_uri: str) -> str:
    return f"""<footer class="foot">
  <span>Served from a mirror · content signed by the collective, not this host</span>
  <span>Verified copy? <a href="{namespace_uri}">Open in Riot →</a></span>
</footer>"""


def render_post(post: PostView, css: str = NEWSPRINT_CSS) -> str:
    namespace_uri = f"riot://open?namespace={post.namespace}"
    status = "Signed by the collective" if post.verified else "Open · unverified"
    ai = '<span>·</span><span class="ai">AI-assisted</span>' if post.ai_assisted else ""
    body = f"""{_page_masthead(post.space_name)}
<main class="article">
  <a class="back" href="/">← {escape(post.space_name)}</a>
  <span class="kicker">{escape(status)}</span>
  <h1>{escape(post.title)}</h1>
  <div class="byline"><span class="who">{escape(post.author)}</span>{ai}</div>
  <div class="article__body"><p>{escape(post.body)}</p></div>
  <div class="provenance">
    <div><b>entry id</b> <span class="mono">{escape(post.entry_id)}</span></div>
    <div><b>author key</b> <span class="mono">{escape(post.signer)}</span></div>
    <div><b>status</b> {escape(status)}</div>
    <div><a href="{namespace_uri}">Verify this record in Riot →</a></div>
  </div>
</main>
{_page_footer(namespace_uri)}"""
    return _doc(f"{post.title} · {post.space_name}", css, body)


def render_author(author: AuthorView, css: str = NEWSPRINT_CSS) -> str:
    namespace_uri = f"riot://open?namespace={author.namespace}"
    role = "recognized organizer" if author.is_organizer else "contributor"
    if author.posts:
        items = "".join(
            f'<li><a href="/post/{escape(p.entry_id)}/">'
            f'{"✓ " if p.verified else ""}{escape(p.title)}</a></li>'
            for p in author.posts
        )
    else:
        items = "<li>Nothing published yet.</li>"
    body = f"""{_page_masthead(author.space_name)}
<main class="narrow">
  <a class="back" href="/">← {escape(author.space_name)}</a>
  <h1>{escape(author.display_name)}</h1>
  <p class="role">{escape(role)} · {author.contribution_count} contributions</p>
  <ol class="feed">{items}</ol>
</main>
{_page_footer(namespace_uri)}"""
    return _doc(f"{author.display_name} · {author.space_name}", css, body)


def sample_view() -> NewswireView:
    """DEMO data — not signed, not from any namespace. Drives the layout until
    real E/W content lands (composite Unit 1/2)."""
    return NewswireView(
        name="RIOT",
        tagline="Independent Newswire · publish anywhere",
        namespace="sample0000000000000000000000000000000000000000000000000000000000",
        categories=("Latest", "Housing", "Labor", "Surveillance", "Ecology", "Repression"),
        mirror_note="live · 41 mirrors reachable · updated 2 min ago",
        editorial=(
            EditorialEntry("Housing · dispatch", "Rent strike jumps three more blocks as tenants tear up eviction notices",
                           "Organizers on Sonnenallee say 400 households are now withholding rent — the largest coordinated action since the 2023 deposit fight. The tenant union answered with a legal-defense phone tree and a block-by-block eviction watch.",
                           "@tenant_union", "15 Jul 18:40"),
            EditorialEntry("Labor", "Port workers walk out in solidarity; container terminal at a standstill",
                           "The wildcat action began at the night shift. Cranes idle, 6,000 boxes stranded. Dockers hold the gate until the fired stewards are reinstated.",
                           "@dockside", "16:12"),
            EditorialEntry("Surveillance", "Leaked procurement docs show the city quietly bought facial-recognition vans",
                           "Four unmarked units, invoiced under \"traffic safety.\" The contract and vendor spec sheet are published in full.",
                           "@freedomofinfo", "14:03"),
            EditorialEntry("Ecology", "Forest occupation enters day 200 as clearing machines pull back",
                           "The tree-village held through the winter. Today the excavators withdrew to the access road — a pause, not a victory.",
                           "@waldbesetzung", "11:47"),
            EditorialEntry("Repression", "Court throws out mass-arrest charges from the May bridge blockade",
                           "Judge finds the kettle unlawful; 88 cases dismissed. The solidarity fund needs legal observers this week.",
                           "@ea_legal", "09:20"),
        ),
        wire=(
            WirePost("18:52", "@kreuzberg_ant", "cops massing at the north gate, maybe 40 vans. bring water."),
            WirePost("18:49", "@anon", "medic station open at the old library, side entrance."),
            WirePost("18:41", "@m.", "bus 12 rerouted, whole ring is blocked. walk from the canal."),
            WirePost("18:33", "@dockside", "second gate just joined the walkout"),
            WirePost("18:20", "@anon", "legal-obs needed at revier 21, two people held."),
            WirePost("17:44", "@anon", "drone overhead on sonnenallee, circling the strike blocks."),
        ),
        sample=True,
    )
