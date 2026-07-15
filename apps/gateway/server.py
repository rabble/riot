#!/usr/bin/env python3
"""Serve the fixed public Riot export locally or behind a future host."""

from __future__ import annotations

import argparse
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from riot_gateway import (
    DEFAULT_EXPORT_PATH,
    DEFAULT_SKIN,
    GatewayError,
    PublicGateway,
    SITE_ROUTES,
    SKINS,
    content_security_policy,
)


def _security_headers(skin: str) -> tuple[tuple[str, str], ...]:
    return (
        ("Content-Security-Policy", content_security_policy(skin)),
        ("X-Content-Type-Options", "nosniff"),
        ("Referrer-Policy", "no-referrer"),
    )


def make_handler(gateway: PublicGateway, skin: str = DEFAULT_SKIN) -> type[BaseHTTPRequestHandler]:
    security_headers = _security_headers(skin)

    class GatewayHandler(BaseHTTPRequestHandler):
        def end_headers(self) -> None:
            for name, value in security_headers:
                self.send_header(name, value)
            super().end_headers()

        def do_GET(self) -> None:  # noqa: N802 - HTTP method name is stdlib API
            try:
                page = gateway.render(self.path, skin)
            except GatewayError:
                self.send_error(HTTPStatus.NOT_FOUND, "public route not found")
                return
            body = page.encode("utf-8")
            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "public, max-age=300")
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self) -> None:  # noqa: N802 - HTTP method name is stdlib API
            self.send_error(HTTPStatus.METHOD_NOT_ALLOWED, "this public reader has no write API")

        do_PUT = do_POST
        do_PATCH = do_POST
        do_DELETE = do_POST

        def log_message(self, _format: str, *_args: object) -> None:
            return

    return GatewayHandler


def dump_site(gateway: PublicGateway, out_dir: Path, skin: str = DEFAULT_SKIN) -> list[Path]:
    """Render every public site route to a static, mirrorable HTML tree.

    Each route lands at ``<out_dir>/<route>/index.html`` so absolute-path links
    resolve on any static host. The output is plain HTML — no server, no trust:
    the mirror is reach, the Riot app is the source of truth.
    """
    written: list[Path] = []
    for route in sorted(SITE_ROUTES):
        page = gateway.render(route, skin)
        path = out_dir / route.strip("/") / "index.html"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(page, encoding="utf-8")
        written.append(path)
    return written


def main() -> None:
    parser = argparse.ArgumentParser(description="Serve the fixed Riot public export")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--export", type=Path, default=DEFAULT_EXPORT_PATH)
    parser.add_argument(
        "--dump",
        type=Path,
        metavar="DIR",
        help="render the site to static HTML in DIR and exit (mirror it anywhere)",
    )
    parser.add_argument(
        "--skin",
        default=DEFAULT_SKIN,
        choices=sorted(SKINS),
        help="which shipped default look to render",
    )
    args = parser.parse_args()
    gateway = PublicGateway.from_file(args.export)
    if args.dump is not None:
        for path in dump_site(gateway, args.dump, args.skin):
            print(f"wrote {path}")
        return
    server = ThreadingHTTPServer((args.host, args.port), make_handler(gateway, args.skin))
    print(f"Riot public gateway serving http://{args.host}:{args.port}/site/")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
