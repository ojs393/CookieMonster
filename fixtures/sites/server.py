#!/usr/bin/env python3
"""Local fixture server for Cookie Monster Observe smoke.

Serves the HTML pages in this directory and emits a synthetic first-party
cookie on GET /set:

    Set-Cookie: cm_test=1; Path=/

No real user cookies are stored or served. Bind defaults to 127.0.0.1.
"""

from __future__ import annotations

import argparse
import sys
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parent
SET_COOKIE = "cm_test=1; Path=/"
DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 8765


class FixtureHandler(SimpleHTTPRequestHandler):
    """Static files plus the /set header used by first-party.html."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

    def do_GET(self) -> None:
        if urlparse(self.path).path == "/set":
            self.send_response(302)
            self.send_header("Set-Cookie", SET_COOKIE)
            self.send_header("Location", "/first-party.html")
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            return
        super().do_GET()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Cookie Monster synthetic fixture server")
    parser.add_argument("--host", default=DEFAULT_HOST, help="bind address (default 127.0.0.1)")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help="bind port (default 8765)")
    args = parser.parse_args(argv)

    ThreadingHTTPServer.allow_reuse_address = True
    httpd = ThreadingHTTPServer((args.host, args.port), FixtureHandler)
    print(
        f"Fixture server at http://{args.host}:{args.port}/\n"
        f"  /set                -> {SET_COOKIE}\n"
        f"  /first-party.html   -> HTTP Set-Cookie landing page\n"
        f"  /js-cookie.html     -> document.cookie page\n"
        "Ctrl-C to stop.",
        flush=True,
    )
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped.", flush=True)
    finally:
        httpd.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
