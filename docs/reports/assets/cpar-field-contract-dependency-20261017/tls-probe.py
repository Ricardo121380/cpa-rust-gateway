"""Loopback only; subprocess-local test CA, no verification bypass or global trust write."""
import hashlib
import json
import os
import select
from pathlib import Path
import socket
import ssl
import subprocess
import sys
import threading
import time

os.umask(0o077)
binary = Path(sys.argv[1]).resolve()
work = Path(sys.argv[2]).resolve()
work.mkdir(exist_ok=True)

def openssl(*args):
    subprocess.run(['openssl', *args], cwd=work, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

openssl('req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', 'ca.key', '-out', 'ca.pem', '-days', '1', '-subj', '/CN=Synthetic Local CA', '-addext', 'basicConstraints=critical,CA:TRUE')
openssl('req', '-new', '-newkey', 'rsa:2048', '-nodes', '-keyout', 'server.key', '-out', 'server.csr', '-subj', '/CN=relay.test')
(work/'server.ext').write_text('subjectAltName=DNS:relay.test\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')
openssl('x509', '-req', '-in', 'server.csr', '-CA', 'ca.pem', '-CAkey', 'ca.key', '-CAcreateserial', '-out', 'server.pem', '-days', '1', '-extfile', 'server.ext')
openssl('req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', 'wrong-ca.key', '-out', 'wrong-ca.pem', '-days', '1', '-subj', '/CN=Unrelated Synthetic CA', '-addext', 'basicConstraints=critical,CA:TRUE')
(work/'empty-certs').mkdir(exist_ok=True)
records = []
cases = [('tls12', ssl.TLSVersion.TLSv1_2, 'relay.test', True, 'ca.pem'),
         ('wrong-host', ssl.TLSVersion.TLSv1_2, 'wrong.test', False, 'ca.pem'),
         ('untrusted-ca', ssl.TLSVersion.TLSv1_2, 'relay.test', False, 'wrong-ca.pem')]
for name, version, host, success, ca in cases:
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.minimum_version = ctx.maximum_version = version
    ctx.load_cert_chain(work/'server.pem', work/'server.key')
    listener = socket.socket(); listener.bind(('127.0.0.1', 0)); listener.listen(2); listener.settimeout(4)
    port = listener.getsockname()[1]
    row = {'case': name, 'tcp_accepts': 0, 'http_requests': 0, 'tls_version': None}
    def serve():
        try:
            peer, _ = listener.accept(); row['tcp_accepts'] += 1
            with peer:
                try:
                    with ctx.wrap_socket(peer, server_side=True) as conn:
                        row['tls_version'] = conn.version()
                        conn.settimeout(3)
                        received = b''
                        while b'\r\n\r\n' not in received and len(received) < 16384:
                            chunk = conn.recv(1024)
                            if not chunk: break
                            received += chunk
                        if b'POST /synthetic HTTP/1.1\r\n' in received:
                            row['http_requests'] += 1
                            body = b'{"synthetic":"ok"}'
                            conn.sendall(b'HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: '+str(len(body)).encode()+b'\r\nConnection: close\r\n\r\n'+body)
                except ssl.SSLError:
                    row['tls_rejected'] = True
            # There must not be a hidden second TCP attempt after success/failure.
            listener.settimeout(0.3)
            try:
                extra, _ = listener.accept(); row['tcp_accepts'] += 1; extra.close()
            except socket.timeout:
                pass
        except Exception as error:
            row['server_error_type'] = type(error).__name__
        finally:
            listener.close()
    thread = threading.Thread(target=serve); thread.start()
    env = os.environ.copy(); env['SSL_CERT_FILE'] = str(work/ca); env['SSL_CERT_DIR'] = str(work/'empty-certs')
    result = subprocess.run([str(binary), str(port), host, 'success' if success else 'reject'], capture_output=True, env=env, timeout=7)
    thread.join(timeout=5)
    row['client_exit'] = result.returncode
    row['client_result'] = result.stdout.decode().strip()
    row['client_stderr_sha256'] = hashlib.sha256(result.stderr).hexdigest()
    row['status'] = 'PASS' if result.returncode == 0 and row['tcp_accepts'] == 1 and row['http_requests'] == int(success) and not thread.is_alive() else 'FAIL'
    records.append(row)

# macOS's system Python uses LibreSSL without TLS 1.3. Exercise TLS 1.3 with the
# available OpenSSL 3 server; its command forces that version, without a trust bypass.
reserved = socket.socket(); reserved.bind(('127.0.0.1', 0)); port = reserved.getsockname()[1]; reserved.close()
server = subprocess.Popen(['openssl', 's_server', '-accept', '127.0.0.1:'+str(port), '-cert', str(work/'server.pem'), '-key', str(work/'server.key'), '-tls1_3', '-brief', '-naccept', '1'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
row = {'case': 'tls13-openssl3', 'http_requests': 0, 'tls_version': None, 'server_accept_limit': 1}
def exchange():
    data = b''; deadline = time.monotonic()+6
    while b'\r\n\r\n' not in data and len(data) < 16384 and time.monotonic() < deadline:
        if select.select([server.stdout], [], [], 0.1)[0]:
            chunk = os.read(server.stdout.fileno(), 1024)
            if not chunk: break
            data += chunk
    if b'POST /synthetic HTTP/1.1\r\n' in data:
        row['http_requests'] = 1
        body = b'{"synthetic":"ok"}'
        server.stdin.write(b'HTTP/1.1 200 OK\r\nContent-Length: '+str(len(body)).encode()+b'\r\nConnection: close\r\n\r\n'+body); server.stdin.flush()
thread = threading.Thread(target=exchange); thread.start()
time.sleep(0.15)
env = os.environ.copy(); env['SSL_CERT_FILE'] = str(work/'ca.pem'); env['SSL_CERT_DIR'] = str(work/'empty-certs')
result = subprocess.run([str(binary), str(port), 'relay.test', 'success'], capture_output=True, env=env, timeout=7)
thread.join(timeout=7)
server.stdin.close(); server.wait(timeout=5)
stderr = server.stderr.read()
row['tls_version'] = 'TLSv1.3' if b'TLSv1.3' in stderr else None
row['server_stderr_sha256'] = hashlib.sha256(stderr).hexdigest()
row['client_exit'] = result.returncode; row['client_result'] = result.stdout.decode().strip()
row['client_stderr_sha256'] = hashlib.sha256(result.stderr).hexdigest()
row['status'] = 'PASS' if result.returncode == 0 and row['http_requests'] == 1 and row['tls_version'] == 'TLSv1.3' and not thread.is_alive() else 'FAIL'
records.append(row)
summary = {'status': 'PASS' if all(x['status'] == 'PASS' for x in records) else 'FAIL', 'production_boundary': 'UpstreamClientPool::send/Standard reqwest DNS-pinned path', 'provider_calls': 0, 'global_trust_writes': 0, 'verification_disabled': False, 'records': records}
(work/'receipt.json').write_text(json.dumps(summary, indent=2)+'\n')
print(json.dumps(summary, indent=2))
sys.exit(0 if summary['status'] == 'PASS' else 1)
