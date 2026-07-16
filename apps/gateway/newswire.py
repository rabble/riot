#!/usr/bin/env python3
"""Navigable two-column newswire: editorial features (E) + open wire (W).

Renders a small but real site — front page, per-article detail pages, and
per-category listings — all self-contained and CSP-fenced in the reach-layer
style (`docs/superpowers/specs/2026-07-16-web-viewer-reach-layer-design.md`).

Decoupled from the hash-locked conference gateway and the Rust core.
`sample_view()` returns clearly-flagged DEMO data (footer: "sample content, not
signed") so the layout works before real signed E/W content exists (composite
Unit 1/2). Same fences everywhere: inline CSS, no external anything,
`default-src 'none'`, deep link to the app for the verified copy.
"""

from __future__ import annotations

from dataclasses import dataclass
from html import escape
import re

from riot_gateway import _sri_sha256


@dataclass(frozen=True)
class EditorialEntry:
    """A signed editorial article (namespace E, owned, verified)."""

    category: str          # canonical, matches a nav category, e.g. "Housing"
    kicker: str            # display eyebrow, e.g. "Housing · dispatch"
    title: str
    summary: str
    body: tuple[str, ...]  # full article, one string per paragraph
    author: str
    time: str
    slug: str
    verified: bool = True


@dataclass(frozen=True)
class WirePost:
    """An open-published post (namespace W, communal, unverified)."""

    time: str
    handle: str
    body: str


@dataclass(frozen=True)
class NewswireView:
    name: str
    tagline: str
    namespace: str
    categories: tuple[str, ...]   # first is "Latest" (the front page)
    editorial: tuple[EditorialEntry, ...]
    wire: tuple[WirePost, ...]
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
.wordmark a { text-decoration: none; }
.masthead__tag { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.18em; color: var(--muted); }
.masthead__meta { margin-left: auto; text-align: right; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.66rem; color: var(--muted); line-height: 1.5; }
.cats { display: flex; gap: 0 1.1rem; flex-wrap: wrap; margin: 0.85rem 0 0; padding: 0.5rem 0; border-top: 1px solid var(--line); list-style: none; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.74rem; letter-spacing: 0.05em; text-transform: uppercase; }
.cats a { text-decoration: none; color: var(--muted); padding-bottom: 2px; border-bottom: 2px solid transparent; }
.cats a:hover, .cats a:focus-visible { color: var(--ink); border-bottom-color: var(--red); }
.cats a.on { color: var(--ink); border-bottom-color: var(--red); }
.shell { max-width: 78rem; margin: 0 auto; padding: 1.5rem 1.25rem 4rem; display: grid; grid-template-columns: minmax(0,1fr) 21rem; gap: 2.5rem; }
.section-label { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.14em; color: var(--muted); margin: 0 0 1.1rem; display: flex; align-items: center; gap: 0.6rem; }
.section-label::after { content: ""; flex: 1; height: 1px; background: var(--line); }
.kicker { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.7rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.1em; color: var(--wire); }
.headline-link { text-decoration: none; }
.headline-link:hover, .headline-link:focus-visible { color: var(--red); }
.lede { border-bottom: 1px solid var(--line); padding-bottom: 1.75rem; margin-bottom: 1.75rem; }
.lede .kicker { color: var(--red); }
.lede h2 { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: clamp(1.9rem,4.6vw,2.9rem); line-height: 1.03; letter-spacing: -0.02em; margin: 0.5rem 0 0.6rem; }
.lede p { margin: 0 0 0.9rem; font-size: 1.08rem; max-width: 42rem; }
.byline { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; color: var(--muted); display: flex; gap: 0.55rem; flex-wrap: wrap; align-items: center; }
.byline .who { color: var(--ink); }
.verified { color: var(--verify); font-weight: 600; }
.verified::before { content: "\\2713 "; }
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
.article { max-width: 44rem; margin: 0 auto; padding: 1.75rem 1.25rem 4rem; }
.back { display: inline-block; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; letter-spacing: 0.04em; text-transform: uppercase; color: var(--muted); text-decoration: none; margin-bottom: 1.25rem; }
.back:hover, .back:focus-visible { color: var(--red); }
.article h1 { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: clamp(2rem,5vw,3rem); line-height: 1.05; letter-spacing: -0.02em; margin: 0.5rem 0 0.7rem; }
.article__body p { font-size: 1.1rem; margin: 0 0 1.1rem; }
.article__foot { margin-top: 2rem; padding-top: 1rem; border-top: 1px solid var(--line); font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; color: var(--muted); }
.article__foot a { color: var(--red); text-decoration: none; }
.foot { max-width: 78rem; margin: 0 auto; padding: 1.4rem 1.25rem 3rem; border-top: 1px solid var(--line); font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.68rem; color: var(--muted); line-height: 1.6; display: flex; gap: 0.6rem 1.5rem; flex-wrap: wrap; align-items: baseline; }
.foot a { color: var(--ink); }
.foot .sample { color: var(--red); }
@media (max-width: 780px) { .shell { grid-template-columns: 1fr; gap: 1.75rem; } .stories { grid-template-columns: 1fr; } .wire { position: static; } }
:focus-visible { outline: 2px solid var(--red); outline-offset: 2px; }
""".strip()


def _slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def _content_security_policy(css: str) -> str:
    return (
        "default-src 'none'; "
        f"style-src 'sha256-{_sri_sha256(css)}'; "
        "script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'"
    )


def _head(title: str, css: str) -> str:
    csp = _content_security_policy(css)
    return (
        f'<head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="{csp}">'
        f'<meta name="viewport" content="width=device-width, initial-scale=1">'
        f"<title>{escape(title)}</title><style>{css}</style></head>"
    )


def _masthead(view: NewswireView, active: str = "Latest") -> str:
    items = []
    for cat in view.categories:
        href = "/" if cat == "Latest" else f"/c/{_slug(cat)}/"
        cls = ' class="on"' if cat == active else ""
        items.append(f'<li><a{cls} href="{href}">{escape(cat)}</a></li>')
    return f"""<header class="masthead">
  <div class="masthead__top">
    <h1 class="wordmark"><a href="/">{escape(view.name)}</a></h1>
    <span class="masthead__tag">{escape(view.tagline)}</span>
    <div class="masthead__meta">live · 41 mirrors reachable · updated 2 min ago</div>
  </div>
  <ul class="cats">{''.join(items)}</ul>
</header>"""


def _footer(view: NewswireView) -> str:
    uri = f"riot://open?namespace={view.namespace}"
    sample = '<span class="sample">demo · sample content, not signed</span>' if view.sample else ""
    return f"""<footer class="foot">
  <span>Served from a mirror · content signed by the collective, not this host</span>
  <span>Verified copy? <a href="{uri}">Open in Riot →</a></span>
  {sample}
</footer>"""


def _byline(entry: EditorialEntry, label: str) -> str:
    verify = f'<span class="verified">{escape(label)}</span>' if entry.verified else ""
    return (
        f'<div class="byline"><span class="who">{escape(entry.author)}</span>'
        f'<span>·</span><span>{escape(entry.time)}</span><span>·</span>{verify}</div>'
    )


def _lede(entry: EditorialEntry) -> str:
    return f"""<article class="lede">
  <span class="kicker">{escape(entry.kicker)}</span>
  <h2><a class="headline-link" href="/article/{entry.slug}/">{escape(entry.title)}</a></h2>
  <p>{escape(entry.summary)}</p>
  {_byline(entry, "verified editorial")}
</article>"""


def _story(entry: EditorialEntry) -> str:
    return f"""<article class="story">
  <span class="kicker">{escape(entry.kicker)}</span>
  <h3><a class="headline-link" href="/article/{entry.slug}/">{escape(entry.title)}</a></h3>
  <p>{escape(entry.summary)}</p>
  {_byline(entry, "verified")}
</article>"""


def _post(post: WirePost) -> str:
    return (
        f'<li class="post"><span class="t">{escape(post.time)}</span> '
        f'<span class="h">{escape(post.handle)}</span>'
        f'<span class="body">{escape(post.body)}</span>'
        f'<span class="open">open · unverified</span></li>'
    )


def render_newswire(view: NewswireView, css: str = NEWSPRINT_CSS) -> str:
    editorial = view.editorial
    lede = _lede(editorial[0]) if editorial else ""
    stories = "".join(_story(entry) for entry in editorial[1:])
    posts = "".join(_post(post) for post in view.wire)
    uri = f"riot://open?namespace={view.namespace}"
    return f"""<!doctype html>
<html lang="en">
{_head(f"{view.name} · Newswire", css)}
<body>
{_masthead(view, "Latest")}
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
    <p class="wire__foot"><a href="{uri}">+ Publish to the wire →</a></p>
  </aside>
</main>
{_footer(view)}
</body>
</html>"""


def render_article(view: NewswireView, entry: EditorialEntry, css: str = NEWSPRINT_CSS) -> str:
    paragraphs = "".join(f"<p>{escape(p)}</p>" for p in entry.body)
    uri = f"riot://open?namespace={view.namespace}"
    return f"""<!doctype html>
<html lang="en">
{_head(f"{entry.title} · {view.name}", css)}
<body>
{_masthead(view, entry.category)}
<main class="article">
  <a class="back" href="/">← {escape(view.name)} newswire</a>
  <span class="kicker">{escape(entry.kicker)}</span>
  <h1>{escape(entry.title)}</h1>
  {_byline(entry, "verified editorial")}
  <div class="article__body">{paragraphs}</div>
  <p class="article__foot">Signed editorial · <a href="{uri}">verify in Riot →</a></p>
</main>
{_footer(view)}
</body>
</html>"""


def render_category(view: NewswireView, category: str, css: str = NEWSPRINT_CSS) -> str:
    entries = [e for e in view.editorial if e.category == category]
    stories = "".join(_story(entry) for entry in entries) or "<p>No stories in this category yet.</p>"
    return f"""<!doctype html>
<html lang="en">
{_head(f"{category} · {view.name}", css)}
<body>
{_masthead(view, category)}
<main class="shell">
  <section aria-label="{escape(category)} stories">
    <p class="section-label">{escape(category)} · signed by the collective</p>
    <div class="stories">{stories}</div>
  </section>
</main>
{_footer(view)}
</body>
</html>"""


def sample_view() -> NewswireView:
    """DEMO data — not signed, not from any namespace. Drives the layout until
    real E/W content lands (composite Unit 1/2)."""
    editorial = (
        EditorialEntry(
            "Housing", "Housing · dispatch",
            "Rent strike jumps three more blocks as tenants tear up eviction notices",
            "Organizers on Sonnenallee say 400 households are now withholding rent — the largest coordinated action since the 2023 deposit fight.",
            (
                "Four hundred households across three more blocks joined the rent strike this week, organizers on Sonnenallee said, bringing the total to the largest coordinated tenant action the district has seen since the 2023 deposit fight.",
                "Landlords' lawyers filed for expedited eviction hearings within hours. The tenant union answered with a legal-defense phone tree and a block-by-block eviction watch: neighbours pledged to show up at any door a bailiff approaches.",
                "\"They can send letters. They can't evict a whole street at once,\" one organizer said at a packed courtyard meeting. The union is asking supporters to sign up for the watch rota and to keep the solidarity fund topped up.",
            ),
            "@tenant_union", "15 Jul 18:40", "rent-strike-three-blocks",
        ),
        EditorialEntry(
            "Labor", "Labor",
            "Port workers walk out in solidarity; container terminal at a standstill",
            "The wildcat action began at the night shift. Cranes idle, 6,000 boxes stranded.",
            (
                "The walkout began quietly at the night shift and by dawn the container terminal was at a standstill — cranes idle, some 6,000 boxes stranded on the quay.",
                "Dockers say they will hold the gate until the stewards fired last month are reinstated. Management called the action unlawful; the workers called it solidarity.",
            ),
            "@dockside", "16:12", "port-workers-walkout",
        ),
        EditorialEntry(
            "Surveillance", "Surveillance",
            "Leaked procurement docs show the city quietly bought facial-recognition vans",
            "Four unmarked units, invoiced under \"traffic safety.\" The contract is published in full.",
            (
                "Procurement documents leaked to this newswire show the city bought four facial-recognition camera vans in March, delivered unmarked and invoiced under a \"traffic safety\" budget line.",
                "We are publishing the contract and the vendor's spec sheet in full so residents can see exactly what was bought, and with whose money.",
            ),
            "@freedomofinfo", "14:03", "facial-recognition-vans",
        ),
        EditorialEntry(
            "Ecology", "Ecology",
            "Forest occupation enters day 200 as clearing machines pull back",
            "The tree-village held through the winter. Today the excavators withdrew to the access road.",
            (
                "The tree-village held through the winter, and on day 200 of the occupation the contractor's excavators withdrew to the access road.",
                "Occupiers caution that it is a pause, not a victory: the night watch continues, and the solidarity camp needs dry wood and batteries before the next cold front.",
            ),
            "@waldbesetzung", "11:47", "forest-occupation-day-200",
        ),
        EditorialEntry(
            "Repression", "Repression",
            "Court throws out mass-arrest charges from the May bridge blockade",
            "Judge finds the kettle unlawful; 88 cases dismissed.",
            (
                "A judge dismissed 88 cases arising from the May bridge blockade, finding that the police kettle that swept up the defendants was itself unlawful.",
                "The solidarity fund says a handful of defendants still face separate charges and need legal observers in court this week.",
            ),
            "@ea_legal", "09:20", "bridge-blockade-charges-dropped",
        ),
    )
    return NewswireView(
        name="RIOT",
        tagline="Independent Newswire · publish anywhere",
        namespace="sample0000000000000000000000000000000000000000000000000000000000",
        categories=("Latest", "Housing", "Labor", "Surveillance", "Ecology", "Repression"),
        editorial=editorial,
        wire=(
            WirePost("18:52", "@kreuzberg_ant", "cops massing at the north gate, maybe 40 vans. bring water."),
            WirePost("18:49", "@anon", "medic station open at the old library, side entrance."),
            WirePost("18:41", "@m.", "bus 12 rerouted, whole ring is blocked. walk from the canal."),
            WirePost("18:33", "@dockside", "second gate just joined the walkout."),
            WirePost("18:20", "@anon", "legal-obs needed at revier 21, two people held."),
            WirePost("17:44", "@anon", "drone overhead on sonnenallee, circling the strike blocks."),
        ),
        sample=True,
    )
