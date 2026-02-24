#!/usr/bin/env python3
"""
Simple HTTP server for KPIO end-to-end I/O testing.

Usage:
    python tests/e2e/http-server.py [--port PORT]

This server is intended to be run on the host machine while QEMU is running
with user-mode networking (SLIRP).  The guest can reach the host at 10.0.2.2.

Endpoints:
    GET  /test          -> 200  {"status": "ok", "message": "KPIO E2E test"}
    GET  /health        -> 200  "healthy"
    POST /echo          -> 200  echoes the request body
    *    (anything else) -> 404

Start this before running `.\scripts\qemu-test.ps1 -Mode io`.
"""

import argparse
import json
from http.server import HTTPServer, BaseHTTPRequestHandler


class E2EHandler(BaseHTTPRequestHandler):
    """Minimal handler for KPIO integration tests."""

    def do_GET(self):
        if self.path == "/test":
            body = json.dumps({
                "status": "ok",
                "message": "KPIO E2E test",
                "version": "9.5",
            }).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/health":
            body = b"healthy"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path == "/echo":
            length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(length) if length else b""
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_error(404)

    def log_message(self, fmt, *args):
        print(f"[E2E HTTP] {args[0]} {args[1]} {args[2]}")


def main():
    parser = argparse.ArgumentParser(description="KPIO E2E test HTTP server")
    parser.add_argument("--port", type=int, default=8080,
                        help="Port to listen on (default: 8080)")
    args = parser.parse_args()

    server = HTTPServer(("0.0.0.0", args.port), E2EHandler)
    print(f"[E2E HTTP] Listening on 0.0.0.0:{args.port}")
    print(f"[E2E HTTP] Guest can reach this at http://10.0.2.2:{args.port}/test")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[E2E HTTP] Stopped")
        server.server_close()


if __name__ == "__main__":
    main()
