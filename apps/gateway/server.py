#!/usr/bin/env python3
"""Serve the fixed public Riot export locally or behind a future host."""

from __future__ import annotations

import argparse
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from riot_gateway import DEFAULT_EXPORT_PATH, GatewayError, PublicGateway


SECURITY_HEADERS = (
    (
        "Content-Security-Policy",
        "default-src 'none'; script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'",
    ),
    ("X-Content-Type-Options", "nosniff"),
    ("Referrer-Policy", "no-referrer"),
)


def make_handler(gateway: PublicGateway) -> type[BaseHTTPRequestHandler]:
    class GatewayHandler(BaseHTTPRequestHandler):
        def end_headers(self) -> None:
            for name, value in SECURITY_HEADERS:
                self.send_header(name, value)
            super().end_headers()

        def do_GET(self) -> None:  # noqa: N802 - HTTP method name is stdlib API
            try:
                page = gateway.render(self.path)
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


def main() -> None:
    parser = argparse.ArgumentParser(description="Serve the fixed Riot public export")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--export", type=Path, default=DEFAULT_EXPORT_PATH)
    args = parser.parse_args()
    gateway = PublicGateway.from_file(args.export)
    server = ThreadingHTTPServer((args.host, args.port), make_handler(gateway))
    print(f"Riot public gateway serving http://{args.host}:{args.port}/site/")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
