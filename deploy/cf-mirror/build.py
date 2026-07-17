#!/usr/bin/env python3
"""Render the reach-layer dump into ./dist for the Cloudflare mirror.

Single source of truth is the Python gateway/newswire renderers; this just
freezes their output into static files the worker serves. Re-run after any
render change: `python3 build.py`.
"""

from __future__ import annotations

import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
GATEWAY = REPO_ROOT / "apps" / "gateway"
sys.path.insert(0, str(GATEWAY))

import newswire as nw  # noqa: E402
import riot_gateway  # noqa: E402
import server  # noqa: E402

# The real signed /2 newswire export produced by `cargo xtask export-newswire`.
NEWSWIRE_EXPORT = REPO_ROOT / "fixtures" / "newswire" / "gateway-space" / "public-export-v1.json"


def _write(path: Path, html: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(html, encoding="utf-8")


def build_newswire(dist: Path, newswire_export: Path = NEWSWIRE_EXPORT) -> None:
    """Freeze the newswire home + one page per post + one page per named
    contributor, rendered from the REAL signed export. The `/demo/` route always
    carries the flagged sample layout. If the export fixture is absent, the home
    falls back to the demo so the mirror still builds."""
    # /demo = the layout on flagged demo content, always available.
    _write(dist / "demo" / "index.html", nw.render_newswire(nw.sample_view()))

    if not newswire_export.exists():
        _write(dist / "index.html", nw.render_newswire(nw.sample_view()))
        return

    export = json.loads(newswire_export.read_text(encoding="utf-8"))

    # Home = the two-column newswire from real signed /2 records (sample=False,
    # so no demo footer).
    _write(dist / "index.html", nw.render_newswire(nw.newswire_view_from_export(export)))

    # One page per re-verified post.
    for entry_id in nw.all_post_ids(export):
        post = nw.post_view_from_export(export, entry_id)
        if post is not None:
            _write(dist / "post" / entry_id / "index.html", nw.render_post(post))

    # One page per NAMED contributor (nameless communal authors get none).
    for contributor in export.get("contributors", []):
        author = nw.author_view_from_export(export, contributor["author_id"])
        if author is not None:
            _write(
                dist / "author" / contributor["author_id"] / "index.html",
                nw.render_author(author),
            )


def main() -> None:
    dist = HERE / "dist"
    dist.mkdir(exist_ok=True)

    # Home + newswire post/author pages, from the real signed export.
    build_newswire(dist)

    # /board = the incident-board dump, both skins, to exercise the vendored
    # client filter and the skin/CSP seam on a live host.
    gateway = riot_gateway.PublicGateway.from_file(riot_gateway.DEFAULT_EXPORT_PATH)
    for skin in ("newsprint", "zine"):
        server.dump_site(gateway, dist / "board" / skin, skin)

    print(f"built dist/ at {dist}")
    for path in sorted(dist.rglob("*.html")):
        print(f"  {path.relative_to(dist)}")


if __name__ == "__main__":
    main()
