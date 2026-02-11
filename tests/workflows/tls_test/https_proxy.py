#!/usr/bin/env python3
"""Minimal HTTPS reverse proxy for TLS testing.

Usage: python3 https_proxy.py <cert> <key> <listen_port> <upstream_port>

Terminates TLS and forwards requests to an HTTP upstream (torc-server).
"""

import http.server
import ssl
import sys
import urllib.request


def main():
    if len(sys.argv) != 5:
        print(f"Usage: {sys.argv[0]} <cert> <key> <listen_port> <upstream_port>")
        sys.exit(1)

    cert_path, key_path, listen_port, upstream_port = sys.argv[1:]
    listen_port = int(listen_port)
    upstream_port = int(upstream_port)

    class ProxyHandler(http.server.BaseHTTPRequestHandler):
        def _proxy(self):
            url = f"http://localhost:{upstream_port}{self.path}"
            headers = {k: v for k, v in self.headers.items()}
            length = self.headers.get("Content-Length")
            body = self.rfile.read(int(length)) if length else None
            req = urllib.request.Request(
                url, data=body, headers=headers, method=self.command
            )
            try:
                resp = urllib.request.urlopen(req)
                self.send_response(resp.status)
                for k, v in resp.getheaders():
                    if k.lower() != "transfer-encoding":
                        self.send_header(k, v)
                self.end_headers()
                self.wfile.write(resp.read())
            except urllib.error.HTTPError as e:
                self.send_response(e.code)
                for k, v in e.headers.items():
                    if k.lower() != "transfer-encoding":
                        self.send_header(k, v)
                self.end_headers()
                self.wfile.write(e.read())

        do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = _proxy

        def log_message(self, format, *args):
            pass  # Suppress request logging

    server = http.server.HTTPServer(("127.0.0.1", listen_port), ProxyHandler)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert_path, key_path)
    server.socket = ctx.wrap_socket(server.socket, server_side=True)
    print(
        f"HTTPS proxy on https://127.0.0.1:{listen_port}"
        f" -> http://localhost:{upstream_port}"
    )
    server.serve_forever()


if __name__ == "__main__":
    main()
