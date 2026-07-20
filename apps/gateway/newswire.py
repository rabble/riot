#!/usr/bin/env python3
"""Render the newswire from the RIGOROUS signed export of Willow records.

The input is `fixtures/newswire/gateway-space/public-export-v1.json` — the
`riot-public-gateway-export/2` export produced by `cargo xtask export-newswire`
(+ `verify-newswire-export`): flat `entries[]` (each independently signature
re-verified, carrying `verification_status`, `featured`, `editorially_verified`,
`ai_assisted`) plus a `contributors[]` block of signed display-name cards
(`author_id`/`display_name`/`is_organizer`/`contribution_count`). Nothing here
is hand-authored — front page, open wire, authors and verification all come from
signed, re-verified records.

`load_export` normalizes `/2` into the internal render shape via `_from_v2`, so
the page renderers below are unchanged from the original site: only the data
source moved onto the rigorous, reverifiable schema.

Reach-layer fences unchanged: inline CSS, no external anything,
`default-src 'none'`, deep link to the app for the verified copy. Moderation is
honoured upstream: Hidden/Tombstoned posts never reach the export, and any entry
whose signature did not re-verify (`signature_invalid`) is dropped here too.
"""

from __future__ import annotations

from html import escape
import json
from pathlib import Path

from riot_gateway import _sri_sha256

EXPORT_PATH = (
    Path(__file__).resolve().parents[2]
    / "fixtures"
    / "newswire"
    / "gateway-space"
    / "public-export-v1.json"
)


def _from_v2(export: dict) -> dict:
    """Map a `riot-public-gateway-export/2` newswire export into the internal
    render shape (space / front_page / open_wire / contributors with per-post
    `author`). Featured entries are the front page; every non-invalid entry is on
    the open wire (the renderer de-dups). Bylines are the author's signed
    display name ONLY — never a key tag; a card-less author is a nameless open
    contributor. `signature_invalid` entries are dropped."""
    cards = {c["author_id"]: c for c in export.get("contributors", [])}

    def author_of(signer: str) -> dict:
        card = cards.get(signer, {})
        name = card.get("display_name") or "Open contributor"
        return {
            "id": signer,
            "rendered": name,  # display name only — no key tag (owner rule)
            "display_name": name,
            "is_organizer": bool(card.get("is_organizer")),
            "contribution_count": int(card.get("contribution_count", 0)),
        }

    posts: list[dict] = []
    for entry in export.get("entries", []):
        if entry.get("verification_status") == "signature_invalid":
            continue
        posts.append(
            {
                "entry_id": entry.get("entry_id", ""),
                "headline": entry.get("title", ""),
                "body": entry.get("body", ""),
                "verified": bool(entry.get("editorially_verified")),
                "ai_assisted": bool(entry.get("ai_assisted")),
                "tai_j2000_micros": entry.get("tai_j2000_micros", 0),
                "treatment": "Ordinary",  # /2 drops Hidden/Tombstoned pre-export
                "featured": bool(entry.get("featured")),
                "author": author_of(entry.get("signer", "")),
            }
        )

    contributors = [
        {
            "id": card["author_id"],
            "rendered": card.get("display_name", card["author_id"]),
            "display_name": card.get("display_name", card["author_id"]),
            "is_organizer": bool(card.get("is_organizer")),
            "contribution_count": int(card.get("contribution_count", 0)),
        }
        for card in export.get("contributors", [])
    ]

    return {
        "space": {
            "name": export.get("title", "RIOT · Newswire"),
            "namespace": export.get("namespace", ""),
            "summary": "Independent community newswire.",
            "topics": [],
            "languages": [],
            "geographic": [],
        },
        "front_page": [p for p in posts if p["featured"]],
        "open_wire": posts,
        "contributors": contributors,
    }


def load_export(path: Path = EXPORT_PATH) -> dict:
    return _from_v2(json.loads(Path(path).read_text(encoding="utf-8")))


def _visible(posts: list[dict]) -> list[dict]:
    # Moderation-aware: only Ordinary posts render. Hidden/Tombstoned never reach
    # the /2 export, so this is normally a no-op — kept as a defensive floor.
    return [p for p in posts if p.get("treatment", "Ordinary") == "Ordinary"]


def _authors(export: dict) -> dict[str, dict]:
    return {c["id"]: c for c in export.get("contributors", [])}


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
.shell { max-width: 78rem; margin: 0 auto; padding: 1.5rem 1.25rem 4rem; display: grid; grid-template-columns: minmax(0,1fr) 21rem; gap: 2.5rem; }
.narrow { max-width: 46rem; margin: 0 auto; padding: 1.5rem 1.25rem 4rem; }
.section-label { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.14em; color: var(--muted); margin: 0 0 1.1rem; display: flex; align-items: center; gap: 0.6rem; }
.section-label::after { content: ""; flex: 1; height: 1px; background: var(--line); }
.kicker { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.7rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.1em; color: var(--wire); }
.headline-link { text-decoration: none; }
.headline-link:hover, .headline-link:focus-visible { color: var(--red); }
.who-link { color: var(--ink); text-decoration: none; border-bottom: 1px solid var(--line); }
.who-link:hover, .who-link:focus-visible { border-bottom-color: var(--red); color: var(--red); }
.lede { border-bottom: 1px solid var(--line); padding-bottom: 1.75rem; margin-bottom: 1.75rem; }
.lede .kicker { color: var(--red); }
.lede h2 { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: clamp(1.9rem,4.6vw,2.9rem); line-height: 1.03; letter-spacing: -0.02em; margin: 0.5rem 0 0.6rem; }
.lede p { margin: 0 0 0.9rem; font-size: 1.08rem; max-width: 42rem; }
.byline { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; color: var(--muted); display: flex; gap: 0.55rem; flex-wrap: wrap; align-items: center; }
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
.post a.pl { color: var(--ink); text-decoration: none; }
.post a.pl:hover { color: var(--red); }
.post .who { color: var(--wire); }
.post a.who { border-bottom: 1px solid var(--line); text-decoration: none; }
.post .vmark { color: var(--verify); }
.post .open { display: inline-block; margin-top: 0.3rem; font-size: 0.62rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--muted); border: 1px solid var(--line); padding: 0 0.3rem; border-radius: 2px; }
.wire__foot { padding: 0.7rem 0.9rem; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.68rem; }
.wire__foot a { color: var(--red); font-weight: 600; text-decoration: none; }
.article { max-width: 44rem; margin: 0 auto; padding: 1.75rem 1.25rem 4rem; }
.back { display: inline-block; font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.72rem; letter-spacing: 0.04em; text-transform: uppercase; color: var(--muted); text-decoration: none; margin-bottom: 1.25rem; }
.back:hover { color: var(--red); }
.article h1 { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: clamp(2rem,5vw,3rem); line-height: 1.05; letter-spacing: -0.02em; margin: 0.5rem 0 0.7rem; }
.article__body p { font-size: 1.1rem; margin: 0 0 1.1rem; }
.provenance { margin-top: 2rem; padding: 0.9rem 1rem; border: 1px solid var(--line); background: var(--panel); font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.7rem; color: var(--muted); line-height: 1.7; word-break: break-all; }
.provenance b { color: var(--ink); }
.provenance a { color: var(--red); text-decoration: none; }
.profile__name { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: clamp(1.8rem,4.5vw,2.6rem); margin: 0.4rem 0 0.2rem; }
.profile__handle { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.8rem; color: var(--muted); word-break: break-all; }
.feed { list-style: none; margin: 1rem 0 0; padding: 0; }
.feed li { border-top: 1px solid var(--line); padding: 0.85rem 0; }
.feed .feed__time { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.68rem; color: var(--muted); }
.feed a { text-decoration: none; }
.feed a:hover { color: var(--red); }
.feed .feed__title { font-family: Georgia, "Times New Roman", serif; font-weight: 700; font-size: 1.15rem; }
.lead { font-size: 1.15rem; margin: 0.5rem 0 1.5rem; max-width: 40rem; }
.why { border-left: 3px solid var(--wire); padding: 0.2rem 0 0.2rem 1rem; margin: 0 0 1.75rem; color: var(--muted); font-size: 0.95rem; max-width: 40rem; }
.steps { counter-reset: step; list-style: none; margin: 0; padding: 0; max-width: 42rem; }
.steps li { position: relative; padding: 0 0 1.4rem 3rem; border-left: 1px solid var(--line); margin-left: 1rem; }
.steps li:last-child { border-left-color: transparent; }
.steps li::before { counter-increment: step; content: counter(step); position: absolute; left: -1rem; top: -0.1rem; width: 2rem; height: 2rem; display: grid; place-items: center; background: var(--red); color: var(--paper); border-radius: 50%; font-family: ui-monospace, Menlo, monospace; font-weight: 700; font-size: 0.9rem; }
.steps h3 { font-family: Georgia, "Times New Roman", serif; font-size: 1.2rem; margin: 0 0 0.3rem; }
.steps p { margin: 0; font-size: 0.98rem; }
.cta { display: inline-block; margin: 0.4rem 0 0; font-family: ui-monospace, Menlo, monospace; font-weight: 600; text-decoration: none; color: var(--paper); background: var(--ink); padding: 0.35rem 0.7rem; }
.cta:hover { background: var(--red); }
.tags { display: flex; gap: 0.4rem; flex-wrap: wrap; margin: 0.7rem 0 1.6rem; }
.tag { font-family: ui-monospace, Menlo, monospace; font-size: 0.68rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); border: 1px solid var(--line); padding: 0.12rem 0.5rem; border-radius: 2px; }
.about h2 { font-family: Georgia, "Times New Roman", serif; font-weight: 800; font-size: 1.5rem; margin: 2.25rem 0 0.6rem; border-top: 2px solid var(--ink); padding-top: 1.4rem; }
.point { margin: 0 0 1.3rem; max-width: 42rem; }
.point b { display: block; font-family: ui-monospace, Menlo, monospace; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--red); margin-bottom: 0.2rem; }
.editors { list-style: none; margin: 0.4rem 0 1.5rem; padding: 0; display: flex; gap: 0.5rem 1.2rem; flex-wrap: wrap; font-family: ui-monospace, Menlo, monospace; font-size: 0.8rem; }
.editors a { color: var(--ink); text-decoration: none; border-bottom: 1px solid var(--line); }
.editors a:hover { color: var(--red); border-bottom-color: var(--red); }
.ident { font-family: ui-monospace, Menlo, monospace; font-size: 0.7rem; color: var(--muted); word-break: break-all; margin: 0.4rem 0 0; }
.foot { max-width: 78rem; margin: 0 auto; padding: 1.4rem 1.25rem 3rem; border-top: 1px solid var(--line); font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 0.68rem; color: var(--muted); line-height: 1.6; display: flex; gap: 0.6rem 1.5rem; flex-wrap: wrap; align-items: baseline; }
.foot a { color: var(--ink); }
.foot .sample { color: var(--red); }
@media (max-width: 780px) { .shell { grid-template-columns: 1fr; gap: 1.75rem; } .stories { grid-template-columns: 1fr; } .wire { position: static; } }
:focus-visible { outline: 2px solid var(--red); outline-offset: 2px; }
""".strip()


def _csp(css: str) -> str:
    return (
        "default-src 'none'; "
        f"style-src 'sha256-{_sri_sha256(css)}'; "
        "script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'"
    )


def _head(title: str, css: str) -> str:
    return (
        f'<head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="{_csp(css)}">'
        f'<meta name="viewport" content="width=device-width, initial-scale=1">'
        f"<title>{escape(title)}</title><style>{css}</style></head>"
    )


def _masthead(export: dict) -> str:
    space = export["space"]
    return f"""<header class="masthead">
  <div class="masthead__top">
    <h1 class="wordmark"><a href="/">{escape(space['name'])}</a></h1>
    <span class="masthead__tag">projected from signed Willow records</span>
    <div class="masthead__meta">live · 41 mirrors reachable · namespace {escape(space['namespace'][:12])}…</div>
  </div>
</header>"""


def _footer(export: dict) -> str:
    uri = f"riot://open?namespace={export['space']['namespace']}"
    return f"""<footer class="foot">
  <span><a href="/about/">About · how this works</a></span>
  <span>Served from a mirror · content signed by the collective, not this host</span>
  <span>Verified copy? <a href="{uri}">Open in Riot →</a></span>
  <span class="sample">demo instance · generated from signed records</span>
</footer>"""


def _author_ref(post_or_id, cls: str = "who-link") -> str:
    author = post_or_id["author"] if isinstance(post_or_id, dict) and "author" in post_or_id else post_or_id
    aid = author["id"]
    return f'<a class="{cls}" href="/author/{aid}/">{escape(author["rendered"])}</a>'


def _byline(post: dict, feature: str) -> str:
    vmark = f'<span class="verified">{escape(feature)}</span>' if post.get("verified") else ""
    return f'<div class="byline">{_author_ref(post)}<span>·</span>{vmark}</div>'


def _lede(post: dict) -> str:
    return f"""<article class="lede">
  <span class="kicker">Featured</span>
  <h2><a class="headline-link" href="/post/{post['entry_id']}/">{escape(post.get('headline') or '')}</a></h2>
  <p>{escape(post.get('body') or '')}</p>
  {_byline(post, "verified editorial")}
</article>"""


def _story(post: dict) -> str:
    return f"""<article class="story">
  <span class="kicker">Featured</span>
  <h3><a class="headline-link" href="/post/{post['entry_id']}/">{escape(post.get('headline') or '')}</a></h3>
  <p>{escape(post.get('body') or '')}</p>
  {_byline(post, "verified")}
</article>"""


def _wire_row(post: dict) -> str:
    vmark = ' <span class="vmark">✓</span>' if post.get("verified") else ""
    return (
        f'<li class="post"><a class="pl" href="/post/{post["entry_id"]}/">'
        f'{escape(post.get("headline") or post.get("body") or "")}</a>{vmark}<br>'
        f'{_author_ref(post, cls="who")}'
        f'<span class="open">open · unverified</span></li>'
    )


def render_newswire(export: dict, css: str = NEWSPRINT_CSS) -> str:
    featured = _visible(export.get("front_page", []))
    featured_ids = {p["entry_id"] for p in featured}
    wire = [p for p in _visible(export.get("open_wire", [])) if p["entry_id"] not in featured_ids]
    lede = _lede(featured[0]) if featured else ""
    stories = "".join(_story(p) for p in featured[1:])
    rows = "".join(_wire_row(p) for p in wire)
    return f"""<!doctype html>
<html lang="en">
{_head(export['space']['name'], css)}
<body>
{_masthead(export)}
<main class="shell">
  <section aria-label="Featured">
    <p class="section-label">Featured · promoted by editors</p>
    {lede}
    <div class="stories">{stories}</div>
  </section>
  <aside class="wire" aria-label="Open newswire">
    <div class="wire__head"><strong>Open Newswire</strong><span>open publishing · anyone can post</span></div>
    <p class="wire__note">Unverified unless an editor signed a verification. Posted over the p2p network. Read with care.</p>
    <ol>{rows}</ol>
    <p class="wire__foot"><a href="/publish/">+ Publish to the wire →</a></p>
  </aside>
</main>
{_footer(export)}
</body>
</html>"""


def render_post(export: dict, post: dict, css: str = NEWSPRINT_CSS) -> str:
    verified = post.get("verified")
    status = "Verified by an editor" if verified else "Open · unverified"
    body = escape(post.get("body") or "")
    # Per-post deep link carries BOTH the namespace and the entry id, so the app
    # can open this community AND jump to the exact record to re-verify (WS3).
    uri = f"riot://open?namespace={export['space']['namespace']}&entry={post['entry_id']}"
    return f"""<!doctype html>
<html lang="en">
{_head((post.get('headline') or 'Post') + ' · ' + export['space']['name'], css)}
<body>
{_masthead(export)}
<main class="article">
  <a class="back" href="/">← {escape(export['space']['name'])}</a>
  <span class="kicker">{escape(status)}</span>
  <h1>{escape(post.get('headline') or '')}</h1>
  {_byline(post, "verified editorial")}
  <div class="article__body"><p>{body}</p></div>
  <div class="provenance">
    <div><b>entry id</b> {escape(post['entry_id'])}</div>
    <div><b>author</b> {escape(post['author']['rendered'])} · <span>{escape(post['author']['id'])}</span></div>
    <div><b>status</b> {escape(status)} · treatment {escape(post.get('treatment','Ordinary'))}</div>
    <div><a href="{uri}">verify this record in Riot →</a></div>
  </div>
</main>
{_footer(export)}
</body>
</html>"""


def render_author(export: dict, author_id: str, css: str = NEWSPRINT_CSS) -> str:
    contributor = _authors(export).get(author_id, {"id": author_id, "rendered": author_id, "display_name": author_id, "is_organizer": False, "contribution_count": 0})
    all_posts = _visible(export.get("front_page", [])) + [
        p for p in _visible(export.get("open_wire", []))
        if p["entry_id"] not in {q["entry_id"] for q in export.get("front_page", [])}
    ]
    mine = [p for p in all_posts if p["author"]["id"] == author_id]
    items = "".join(
        f'<li><a href="/post/{p["entry_id"]}/"><span class="feed__time">{"✓ verified · " if p.get("verified") else ""}entry {escape(p["entry_id"][:12])}…</span>'
        f'<br><span class="feed__title">{escape(p.get("headline") or p.get("body") or "")}</span></a></li>'
        for p in mine
    ) or "<p>Nothing published yet.</p>"
    role = "recognized organizer" if contributor.get("is_organizer") else "contributor"
    return f"""<!doctype html>
<html lang="en">
{_head(contributor['rendered'] + ' · ' + export['space']['name'], css)}
<body>
{_masthead(export)}
<main class="narrow">
  <a class="back" href="/">← {escape(export['space']['name'])}</a>
  <h1 class="profile__name">{escape(contributor['display_name'])}</h1>
  <div class="profile__handle">{escape(contributor['id'])} · {role} · {contributor.get('contribution_count', 0)} signed records</div>
  <p class="section-label">Published</p>
  <ul class="feed">{items}</ul>
</main>
{_footer(export)}
</body>
</html>"""


def render_about(export: dict, css: str = NEWSPRINT_CSS) -> str:
    space = export["space"]
    uri = f"riot://open?namespace={space['namespace']}"
    topics = "".join(f'<span class="tag">{escape(t)}</span>' for t in space.get("topics", []))
    langs = ", ".join(space.get("languages", []))
    geo = ", ".join(space.get("geographic", []))
    editors = "".join(
        f'<li><a href="/author/{c["id"]}/">{escape(c["rendered"])}</a>'
        f'{" · organizer" if c.get("is_organizer") else ""}</li>'
        for c in export.get("contributors", [])
    ) or "<li>None listed yet.</li>"
    return f"""<!doctype html>
<html lang="en">
{_head(f"About · {space['name']}", css)}
<body>
{_masthead(export)}
<main class="narrow about">
  <a class="back" href="/">← {escape(space['name'])}</a>
  <span class="kicker">About · how this works</span>
  <h1 class="profile__name">{escape(space['name'])}</h1>
  <p class="lead">{escape(space.get('summary', ''))}</p>
  <div class="tags">{topics}</div>
  <p class="ident">Languages: {escape(langs)} · Region: {escape(geo)}<br>Namespace: {escape(space['namespace'])}</p>

  <h2>The collective</h2>
  <p class="point">This newswire is run by the people who publish it — the editors below sign the featured articles; anyone can post to the open wire. There is no company behind it and no server that owns it. Its identity is a cryptographic namespace, not a domain someone can seize.</p>
  <p class="point"><b>Editors</b></p>
  <ul class="editors">{editors}</ul>

  <h2>How Riot beats censorship</h2>
  <p class="point"><b>Many mirrors, not one site</b>This page is one copy of many. Block or seize one and the others stand; anyone can host another in minutes. There is no single address to take down.</p>
  <p class="point"><b>Signed records, checked in the app</b>A mirror can display altered text or false attribution in a browser. It cannot produce an independently synced signed record that Riot accepts as the claimed author. Riot checks record signatures and authorization; that establishes provenance, not whether a claim is true, current, complete, safe, or endorsed.</p>
  <p class="point"><b>Publishing moves peer to peer</b>Publishers use Riot; signed posts travel between phones and volunteer seeds. Peer-to-peer does not mean anonymous: devices and infrastructure may observe connections. There is no single publishing server that owns the newswire.</p>
  <p class="point"><b>Readers stay out of the graph</b>Reading over the plain web — especially through a Tor / onion gateway — keeps you off the peer-to-peer network entirely. You are just someone who loaded a web page, not an identifiable node in the system.</p>
  <p class="point"><b>Verify when it matters</b>When provenance matters, open the story in Riot and read the independently synced record instead of trusting what a mirror displayed. The web provides reach; the app checks the record.</p>

  <p class="point"><a class="cta" href="{uri}">Open this newswire in Riot →</a></p>
</main>
{_footer(export)}
</body>
</html>"""


def render_publish(export: dict, css: str = NEWSPRINT_CSS) -> str:
    uri = f"riot://open?namespace={export['space']['namespace']}"
    return f"""<!doctype html>
<html lang="en">
{_head(f"Publish · {export['space']['name']}", css)}
<body>
{_masthead(export)}
<main class="narrow">
  <a class="back" href="/">← {escape(export['space']['name'])}</a>
  <span class="kicker">Publishing</span>
  <h1 class="profile__name">Publish from the Riot app</h1>
  <p class="lead">You can read this newswire on the web, but you publish from the Riot app — never from a web page.</p>
  <p class="why">By design. Your signing key never touches a browser or this mirror, and web readers stay out of the peer-to-peer graph. The app holds the keys and does the signing; the web is a read-only window. That separation is what keeps publishers safer.</p>
  <ol class="steps">
    <li><h3>Get Riot</h3><p>Install the Riot app on your phone or desktop. It carries your keys and joins the peer-to-peer network directly.</p></li>
    <li><h3>Open this newswire</h3><p>Open the share link in the app, or scan its QR from a poster or another device.</p><a class="cta" href="{uri}">Open this newswire in Riot →</a></li>
    <li><h3>Write and publish</h3><p>Post to the open wire, or — if you're an editor — publish an article. The app signs it with your key.</p></li>
    <li><h3>It syncs to the mirrors</h3><p>Your signed post travels peer-to-peer to seeds and mirrors. Minutes later it appears on every copy of this newswire, including this web one.</p></li>
  </ol>
</main>
{_footer(export)}
</body>
</html>"""


def all_posts(export: dict) -> list[dict]:
    """Every visible post, de-duplicated by entry_id (front_page ⊆ open_wire)."""
    seen: dict[str, dict] = {}
    for p in _visible(export.get("front_page", [])) + _visible(export.get("open_wire", [])):
        seen.setdefault(p["entry_id"], p)
    return list(seen.values())
