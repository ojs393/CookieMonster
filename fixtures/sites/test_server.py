#!/usr/bin/env python3
"""Unit tests for the synthetic fixture HTTP server."""

from __future__ import annotations

import http.client
import threading
import unittest
from http.server import ThreadingHTTPServer

import server


class FixtureServerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        ThreadingHTTPServer.allow_reuse_address = True
        cls.httpd = ThreadingHTTPServer(("127.0.0.1", 0), server.FixtureHandler)
        cls.port = cls.httpd.server_address[1]
        cls.thread = threading.Thread(target=cls.httpd.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.httpd.shutdown()
        cls.httpd.server_close()
        cls.thread.join(timeout=2)

    def _get(self, path: str) -> http.client.HTTPResponse:
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=2)
        conn.request("GET", path)
        return conn.getresponse()

    def test_set_emits_synthetic_cookie_and_redirect(self) -> None:
        response = self._get("/set")
        self.assertEqual(response.status, 302)
        self.assertEqual(response.getheader("Location"), "/first-party.html")
        self.assertEqual(response.getheader("Set-Cookie"), server.SET_COOKIE)
        self.assertEqual(response.getheader("Cache-Control"), "no-store")
        body = response.read()
        self.assertEqual(body, b"")

    def test_index_and_pages_are_served(self) -> None:
        for path in ("/", "/index.html", "/first-party.html", "/js-cookie.html"):
            with self.subTest(path=path):
                response = self._get(path)
                self.assertEqual(response.status, 200)
                # Static pages must not *set* cookies via response headers.
                self.assertIsNone(response.getheader("Set-Cookie"))
                body = response.read().decode("utf-8", errors="replace")
                self.assertTrue(body)

    def test_set_cookie_is_synthetic_only(self) -> None:
        self.assertEqual(server.SET_COOKIE, "cm_test=1; Path=/")
        self.assertNotIn("session", server.SET_COOKIE.lower())


if __name__ == "__main__":
    unittest.main()
