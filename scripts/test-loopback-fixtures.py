#!/usr/bin/env python3
"""Local fixture transport works without reverse DNS or external listeners."""
import http.client
from http.server import BaseHTTPRequestHandler
import importlib.util
from pathlib import Path
import threading
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('local_web', Path(__file__).with_name('test-local-web.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        body = b'fixture'
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class Fixtures(unittest.TestCase):
    def test_numeric_loopback_serves_without_reverse_dns(self):
        with patch('socket.getfqdn', side_effect=AssertionError('fixture must not resolve DNS')) as dns:
            server = m.LoopbackHTTPServer(('127.0.0.1', 0), Handler)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                self.assertEqual(server.server_name, '127.0.0.1')
                self.assertGreater(server.server_port, 0)
                connection = http.client.HTTPConnection('127.0.0.1', server.server_port, timeout=3)
                try:
                    connection.request('GET', '/')
                    response = connection.getresponse()
                    self.assertEqual(response.status, 200)
                    self.assertEqual(response.read(), b'fixture')
                finally:
                    connection.close()
                dns.assert_not_called()
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=3)
                self.assertFalse(thread.is_alive())

    def test_external_or_hostname_fixture_listener_is_rejected(self):
        for address in ('0.0.0.0', 'localhost', 'example.invalid'):
            with self.subTest(address=address), self.assertRaises(m.SmokeFailure):
                m.LoopbackHTTPServer((address, 0), Handler)


if __name__ == '__main__':
    unittest.main()
