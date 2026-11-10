import base64
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import sys
import socket
import threading
import time
from urllib.parse import parse_qs, urlparse

mode = sys.argv[1]
config = json.loads(sys.stdin.readline())
root = Path(config['root'])
assets = {size: (root / f'asset-{size}.bin').read_bytes() for size in (262144, 1048576)}
condition = threading.Condition()
sequence = 0
stopping = threading.Event()
output_lock = threading.Lock()

def emit(value):
    with output_lock:
        sys.stdout.write(json.dumps(value, separators=(',', ':')) + '\n')
        sys.stdout.flush()

def watch():
    global sequence
    previous = (root / 'changed.txt').stat().st_mtime_ns
    while not stopping.wait(.005):
        current = (root / 'changed.txt').stat().st_mtime_ns
        if current != previous:
            previous = current
            with condition:
                sequence += 1
                condition.notify_all()
            if mode == 'stdio':
                emit({'event': 'changed', 'sequence': sequence})

threading.Thread(target=watch, daemon=True).start()

def rpc(request):
    if request['method'] == 'ping':
        return {'id': request['id'], 'result': request['payload']}
    if request['method'] == 'asset':
        return {'id': request['id'], 'result': base64.b64encode(assets[request['size']]).decode('ascii')}
    raise ValueError('unknown method')

if mode == 'stdio':
    emit({'ready': True, 'pid': os.getpid(), 'python': sys.version.split()[0]})
    try:
        for line in sys.stdin:
            request = json.loads(line)
            if request['method'] == 'shutdown':
                emit({'id': request['id'], 'result': 'bye'})
                break
            emit(rpc(request))
    finally:
        stopping.set()
else:
    class Handler(BaseHTTPRequestHandler):
        protocol_version = 'HTTP/1.1'

        def setup(self):
            super().setup()
            self.connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

        def log_message(self, *_):
            pass

        def respond(self, status, body, kind='application/json'):
            self.send_response(status)
            self.send_header('Content-Type', kind)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def authorized(self):
            if self.headers.get('Authorization') != 'Bearer ' + config['token']:
                length = int(self.headers.get('Content-Length', 0))
                if length:
                    self.rfile.read(length)
                self.respond(403, b'{"error":"forbidden"}')
                return False
            return True

        def do_POST(self):
            if not self.authorized():
                return
            request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            self.respond(200, json.dumps(rpc(request), separators=(',', ':')).encode())

        def do_GET(self):
            if not self.authorized():
                return
            url = urlparse(self.path)
            query = parse_qs(url.query)
            if url.path == '/asset':
                self.respond(200, assets[int(query['size'][0])], 'application/octet-stream')
            elif url.path == '/events':
                since = int(query['since'][0])
                with condition:
                    condition.wait_for(lambda: sequence > since or stopping.is_set(), timeout=2)
                    body = json.dumps({'event': 'changed', 'sequence': sequence}, separators=(',', ':')).encode()
                self.respond(200, body)
            else:
                self.respond(404, b'{}')

    class Server(ThreadingHTTPServer):
        daemon_threads = True

    server = Server(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, kwargs={'poll_interval': .01}, daemon=True).start()
    emit({'ready': True, 'port': server.server_address[1], 'pid': os.getpid(), 'python': sys.version.split()[0]})
    try:
        for line in sys.stdin:
            if json.loads(line).get('method') == 'shutdown':
                break
    finally:
        stopping.set()
        with condition:
            condition.notify_all()
        server.shutdown()
        server.server_close()
