"""Одноразовый localhost HTTPS fixture; проверяется отказ Java trust store."""
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
import ssl
import sys


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'{}')


root = Path(sys.argv[1])
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(root / 'cert.pem', root / 'test-key.pem')
server = HTTPServer(('127.0.0.1', 26447), Handler)
server.socket = context.wrap_socket(server.socket, server_side=True)
server.serve_forever()
