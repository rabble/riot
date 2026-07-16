#!/usr/bin/env python3
"""Render the reach-layer dump into ./dist for the Cloudflare mirror.

Single source of truth is the Python gateway/newswire renderers; this just
freezes their output into static files the worker serves. Re-run after any
render change: `python3 build.py`.
"""

from __future__ import annotations

from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
GATEWAY = HERE.parents[1] / "apps" / "gateway"
sys.path.insert(0, str(GATEWAY))

import newswire as nw  # noqa: E402
import riot_gateway  # noqa: E402
import server  # noqa: E402


def main() -> None:
    dist = HERE / "dist"
    dist.mkdir(exist_ok=True)

    # Home = the two-column newswire (E features + W open-wire), demo content.
    view = nw.sample_view()
    (dist / "index.html").write_text(nw.render_newswire(view), encoding="utf-8")

    # Per-article detail pages (headlines link here).
    for entry in view.editorial:
        page = dist / "article" / entry.slug
        page.mkdir(parents=True, exist_ok=True)
        (page / "index.html").write_text(nw.render_article(view, entry), encoding="utf-8")

    # Per-category listings (nav links here).
    for category in view.categories:
        if category == "Latest":
            continue
        page = dist / "c" / nw._slug(category)
        page.mkdir(parents=True, exist_ok=True)
        (page / "index.html").write_text(nw.render_category(view, category), encoding="utf-8")

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
