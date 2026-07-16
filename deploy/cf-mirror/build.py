#!/usr/bin/env python3
"""Render the reach-layer dump into ./dist for the Cloudflare mirror.

Single source of truth is the Python gateway/newswire renderers; this just
freezes their output into static files the worker serves. Re-run after any
render change: `python3 build.py`.
"""

from __future__ import annotations

from pathlib import Path
import shutil
import sys

HERE = Path(__file__).resolve().parent
GATEWAY = HERE.parents[1] / "apps" / "gateway"
sys.path.insert(0, str(GATEWAY))

import newswire as nw  # noqa: E402
import riot_gateway  # noqa: E402
import server  # noqa: E402


def main() -> None:
    dist = HERE / "dist"
    if dist.exists():
        shutil.rmtree(dist)  # no stale pages from a previous build
    dist.mkdir(parents=True)

    # Newswire = a REAL projection of signed Willow records
    # (fixtures/newswire/newswire-export-v1.json, minted by the riot-ffi
    # generator). Regenerate it with:
    #   cargo test -p riot-ffi --test generate_newswire_export -- --ignored
    export = nw.load_export()
    (dist / "index.html").write_text(nw.render_newswire(export), encoding="utf-8")

    # /publish = how to publish (from the app; the web is read-only by design).
    pub = dist / "publish"
    pub.mkdir(parents=True, exist_ok=True)
    (pub / "index.html").write_text(nw.render_publish(export), encoding="utf-8")

    # Post permalinks — keyed by real entry_id (content hash).
    for post in nw.all_posts(export):
        page = dist / "post" / post["entry_id"]
        page.mkdir(parents=True, exist_ok=True)
        (page / "index.html").write_text(nw.render_post(export, post), encoding="utf-8")

    # Author profiles — one per real contributor (signer). Everything they've
    # published aggregates here.
    for contributor in export.get("contributors", []):
        page = dist / "author" / contributor["id"]
        page.mkdir(parents=True, exist_ok=True)
        (page / "index.html").write_text(nw.render_author(export, contributor["id"]), encoding="utf-8")

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
