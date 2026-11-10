import base64
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import platform
import queue
import secrets
import statistics
import subprocess
import sys
import tempfile
import threading
import time

HERE = Path(__file__).resolve().parent
TRIALS = 5
RPC_REPEATS = 100
ASSET_REPEATS = 10
EVENT_REPEATS = 10


def stats(values):
    ordered = sorted(values)
    return {'median_ms': round(statistics.median(values), 4), 'p95_ms': round(ordered[math.ceil(.95 * len(ordered)) - 1], 4), 'min_ms': round(ordered[0], 4), 'max_ms': round(ordered[-1], 4), 'count': len(values)}


class Client:
    def __init__(self, mode, root):
        self.mode = mode
        self.root = root
        self.token = secrets.token_urlsafe(32)
        self.output = queue.Queue()
        self.identifier = 0
        self.events = []
        self.body_bytes = 0
        started = time.perf_counter_ns()
        self.child = subprocess.Popen([sys.executable, str(HERE / 'backend.py'), mode], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        self.child.stdin.write(json.dumps({'root': str(root), 'token': self.token}) + '\n')
        self.child.stdin.flush()
        def collect():
            try:
                for line in self.child.stdout:
                    self.output.put(line)
            finally:
                self.output.put(None)
        threading.Thread(target=collect, daemon=True).start()
        ready = self.line()
        self.startup_ms = (time.perf_counter_ns() - started) / 1e6
        self.port = ready.get('port')
        if self.port:
            self.connection = http.client.HTTPConnection('127.0.0.1', self.port, timeout=3)
        self.body_bytes = 0

    def line(self):
        raw = self.output.get(timeout=3)
        if raw is None:
            raise EOFError('backend stdout closed')
        self.body_bytes += len(raw.encode())
        return json.loads(raw)

    def send(self, request):
        self.child.stdin.write(json.dumps(request, separators=(',', ':')) + '\n')
        self.child.stdin.flush()

    def http(self, path, body=None, token=None, separate=False):
        connection = http.client.HTTPConnection('127.0.0.1', self.port, timeout=3) if separate else self.connection
        headers = {'Authorization': 'Bearer ' + (self.token if token is None else token)}
        try:
            connection.request('POST' if body is not None else 'GET', path, body=body, headers=headers)
            response = connection.getresponse()
            data = response.read()
            self.body_bytes += len(data)
            return response.status, data
        finally:
            if separate:
                connection.close()

    def request(self, method, **args):
        self.identifier += 1
        request = {'id': self.identifier, 'method': method, **args}
        if self.mode == 'stdio':
            self.send(request)
            while True:
                response = self.line()
                if 'event' in response:
                    self.events.append(response)
                else:
                    assert response['id'] == self.identifier
                    return response['result']
        status, body = self.http('/rpc', json.dumps(request, separators=(',', ':')))
        assert status == 200
        response = json.loads(body)
        assert response['id'] == self.identifier
        return response['result']

    def asset(self, size):
        if self.mode == 'stdio':
            return base64.b64decode(self.request('asset', size=size), validate=True)
        status, body = self.http(f'/asset?size={size}')
        assert status == 200
        return body

    def event(self, since):
        if self.mode == 'stdio':
            event = self.events.pop(0) if self.events else self.line()
        else:
            status, body = self.http(f'/events?since={since}')
            assert status == 200
            event = json.loads(body)
        assert event['event'] == 'changed' and event['sequence'] > since
        return event['sequence']

    def stop(self, crash=False, eof=False):
        started = time.perf_counter_ns()
        if crash:
            self.child.kill()
        elif eof:
            self.child.stdin.close()
        else:
            self.send({'method': 'shutdown', 'id': -1})
        self.child.wait(timeout=3)
        if self.mode == 'http':
            self.connection.close()
        return (time.perf_counter_ns() - started) / 1e6

    def cleanup(self):
        if self.child.poll() is None:
            self.child.kill()
            self.child.wait(timeout=3)
        for stream in (self.child.stdin, self.child.stdout, self.child.stderr):
            if stream and not stream.closed:
                try:
                    stream.close()
                except BrokenPipeError:
                    pass


def measure(action, count, validator):
    values = []
    for _ in range(count):
        started = time.perf_counter_ns()
        value = action()
        elapsed = (time.perf_counter_ns() - started) / 1e6
        validator(value)
        values.append(elapsed)
    return values


def run():
    results = {'conditions': {'platform': platform.system(), 'machine': platform.machine(), 'python': platform.python_version(), 'trials': TRIALS, 'rpc_repeats_per_trial': RPC_REPEATS, 'asset_repeats_per_size_per_trial': ASSET_REPEATS, 'notification_repeats_per_trial': EVENT_REPEATS, 'warmup_rpc': 10, 'warmup_assets_per_size': 1, 'watch_interval_ms': 5, 'http_connection': 'persistent HTTP/1.1; backend TCP_NODELAY enabled', 'scope': 'Linux parent to Linux child; excludes Windows/wsl.exe startup, networking and WebView. Synthetic fixture only.', 'timing': 'Round-trip to decoded response (validation/hash excluded); startup separately. Sequential requests; event delay includes 5ms polling. Body bytes exclude HTTP headers/TCP and stdio request bytes.'}, 'trials': [], 'checksums': {}, 'errors': []}
    payload = 'request-' + 'x' * 1024
    with tempfile.TemporaryDirectory(prefix='repo-notes-transport-') as temporary:
        root = Path(temporary)
        expected = {}
        for size in (262144, 1048576):
            data = bytes(range(256)) * (size // 256)
            (root / f'asset-{size}.bin').write_bytes(data)
            expected[size] = hashlib.sha256(data).hexdigest()
        results['checksums'] = {str(k): v for k, v in expected.items()}
        (root / 'changed.txt').write_text('initial')
        for trial in range(TRIALS):
            # Alternate ordering to avoid assigning every first/cold run to stdio.
            for mode in (('stdio', 'http') if trial % 2 == 0 else ('http', 'stdio')):
                client = None
                try:
                    client = Client(mode, root)
                    row = {'trial': trial + 1, 'mode': mode, 'startup_ms': round(client.startup_ms, 4), 'assets': {}, 'correct_responses': 0}
                    def validate_rpc(value):
                        assert value == payload
                        row['correct_responses'] += 1
                    def validate_asset(size, value):
                        assert len(value) == size
                        assert hashlib.sha256(value).hexdigest() == expected[size]
                        row['correct_responses'] += 1
                    for _ in range(10):
                        assert client.request('ping', payload=payload) == payload
                    for size in (262144, 1048576):
                        validate_asset(size, client.asset(size))
                    row['rpc_raw_ms'] = measure(lambda: client.request('ping', payload=payload), RPC_REPEATS, validate_rpc)
                    row['rpc'] = stats(row['rpc_raw_ms'])
                    for size in (262144, 1048576):
                        before = client.body_bytes
                        values = measure(lambda: client.asset(size), ASSET_REPEATS, lambda value: validate_asset(size, value))
                        row['assets'][str(size)] = {**stats(values), 'raw_ms': values, 'response_body_bytes_total': client.body_bytes - before, 'payload_bytes_total': size * ASSET_REPEATS}
                    previous = 0
                    values = []
                    for index in range(EVENT_REPEATS):
                        # Each acknowledged event precedes the next file change.
                        started = time.perf_counter_ns()
                        (root / 'changed.txt').write_text(f'{trial}-{mode}-{index}-{time.monotonic_ns()}')
                        previous = client.event(previous)
                        values.append((time.perf_counter_ns() - started) / 1e6)
                    row['notifications'] = {**stats(values), 'raw_ms': values, 'received': previous}
                    if mode == 'http':
                        row['unauthenticated_status'] = client.http('/asset?size=262144', token='', separate=True)[0]
                        row['wrong_token_status'] = client.http('/asset?size=262144', token='invalid', separate=True)[0]
                        assert row['unauthenticated_status'] == row['wrong_token_status'] == 403
                        connection = http.client.HTTPConnection('127.0.0.1', client.port, timeout=3)
                        try:
                            connection.request('GET', '/asset?size=262144')
                            response = connection.getresponse()
                            response.read()
                            row['missing_auth_header_status'] = response.status
                            assert response.status == 403
                        finally:
                            connection.close()
                        row['second_client_with_token_status'] = client.http('/asset?size=262144', separate=True)[0]
                    else:
                        row['client_isolation'] = 'No listening socket; commands arrive through inherited private stdin. Not a same-OS-user security boundary.'
                    row['shutdown_ms'] = round(client.stop(), 4)
                    row['shutdown_exit'] = client.child.returncode
                    assert client.child.returncode == 0
                    results['trials'].append(row)
                    print(json.dumps({'trial': trial + 1, 'mode': mode, 'startup_ms': row['startup_ms'], 'rpc': row['rpc'], 'assets': {key: {k: v for k, v in value.items() if k != 'raw_ms'} for key, value in row['assets'].items()}, 'notifications': {k: v for k, v in row['notifications'].items() if k != 'raw_ms'}}), flush=True)
                except Exception as error:
                    results['errors'].append({'trial': trial + 1, 'mode': mode, 'type': type(error).__name__, 'message': str(error)})
                    raise
                finally:
                    if client:
                        client.cleanup()
        for mode in ('stdio', 'http'):
            for scenario in ('crash', 'parent_eof'):
                client = Client(mode, root)
                try:
                    elapsed = client.stop(crash=scenario == 'crash', eof=scenario == 'parent_eof')
                    check = {'mode': mode, 'scenario': scenario, 'exit': client.child.returncode, 'stop_ms': round(elapsed, 4)}
                    if scenario == 'crash':
                        try:
                            client.request('ping', payload='after-crash')
                            raise AssertionError('request unexpectedly succeeded')
                        except (BrokenPipeError, EOFError, ConnectionError, OSError, http.client.HTTPException) as error:
                            check['request_after_crash'] = type(error).__name__
                    else:
                        assert client.child.returncode == 0
                    results.setdefault('lifecycle', []).append(check)
                finally:
                    client.cleanup()
    summaries = {}
    for mode in ('stdio', 'http'):
        rows = [row for row in results['trials'] if row['mode'] == mode]
        summaries[mode] = {'startup': stats([row['startup_ms'] for row in rows]), 'shutdown': stats([row['shutdown_ms'] for row in rows]), 'rpc': stats([value for row in rows for value in row['rpc_raw_ms']]), 'notification': stats([value for row in rows for value in row['notifications']['raw_ms']]), 'assets': {str(size): stats([value for row in rows for value in row['assets'][str(size)]['raw_ms']]) for size in (262144, 1048576)}, 'trial_rpc_medians_ms': [row['rpc']['median_ms'] for row in rows]}
    results['summary'] = summaries
    (HERE / 'results.json').write_text(json.dumps(results, indent=2))
    print(json.dumps({'summary': summaries, 'lifecycle': results['lifecycle'], 'errors': results['errors']}, indent=2))


WINDOWS_SCRIPT = r'''
param([string]$Distro, [string]$Root, [string]$Backend)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Net.Http
$rows = [System.Collections.Generic.List[object]]::new()
$payload = 'request-' + ('x' * 1024)
$sha = [System.Security.Cryptography.SHA256]::Create()
function Read-BackendLine {
    $task = $script:process.StandardOutput.ReadLineAsync()
    if (-not $task.Wait(5000)) { throw 'backend read timeout' }
    if ($null -eq $task.Result) { throw 'backend stdout closed' }
    try { return ($task.Result | ConvertFrom-Json) } catch { throw ('Unexpected backend stdout codepoints: ' + (($task.Result.ToCharArray() | ForEach-Object { [int]$_ }) -join ',')) }
}
function Invoke-Rpc([string]$Method, [int]$Size = 0) {
    $script:requestId += 1
    $request = @{id=$script:requestId; method=$Method; payload=$payload; size=$Size}
    $json = $request | ConvertTo-Json -Compress
    if ($script:mode -eq 'stdio') {
        $script:process.StandardInput.WriteLine($json)
        $response = Read-BackendLine
    } else {
        $content = [System.Net.Http.StringContent]::new($json, [System.Text.Encoding]::UTF8, 'application/json')
        try {
            $reply = $script:http.PostAsync($script:url + '/rpc', $content).GetAwaiter().GetResult()
            try {
                if ([int]$reply.StatusCode -ne 200) { throw 'RPC HTTP status' }
                $response = $reply.Content.ReadAsStringAsync().GetAwaiter().GetResult() | ConvertFrom-Json
            } finally { $reply.Dispose() }
        } finally { $content.Dispose() }
    }
    if ($response.id -ne $script:requestId) { throw 'RPC id mismatch' }
    return $response.result
}
function Read-Asset([int]$Size) {
    if ($script:mode -eq 'stdio') {
        $data = [Convert]::FromBase64String((Invoke-Rpc 'asset' $Size))
    } else {
        $data = $script:http.GetByteArrayAsync($script:url + '/asset?size=' + $Size).GetAwaiter().GetResult()
    }
    return ,$data
}
function Validate-Asset([byte[]]$Data, [int]$Size) {
    if ($Data.Length -ne $Size) { throw 'asset size mismatch' }
    $digest = [BitConverter]::ToString($sha.ComputeHash($Data)).Replace('-', '').ToLowerInvariant()
    if ($digest -ne $script:expected[[string]$Size]) { throw 'asset checksum mismatch' }
}
$script:expected = @{'262144'='2312394bd99545d9de131c24efb781e765ac1aec243f2ed9347597a793a415e9'; '1048576'='fbbab289f7f94b25736c58be46a994c441fd02552cc6022352e3d86d2fab7c83'}
for ($trial = 1; $trial -le 5; $trial++) {
    $modes = if ($trial % 2 -eq 1) { @('stdio', 'http') } else { @('http', 'stdio') }
    foreach ($modeName in $modes) {
        $script:mode = $modeName
        $script:requestId = 0
        $script:http = $null
        $script:process = $null
        $tokenBytes = [byte[]]::new(32)
        $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
        $rng.GetBytes($tokenBytes)
        $rng.Dispose()
        $token = [Convert]::ToBase64String($tokenBytes)
        $start = [Diagnostics.Stopwatch]::StartNew()
        try {
            $info = [Diagnostics.ProcessStartInfo]::new()
            $info.FileName = 'C:\Windows\System32\wsl.exe'
            $info.Arguments = '-d ' + $Distro + ' --exec python3 ' + $Backend + ' ' + $mode
            $info.UseShellExecute = $false
            $info.CreateNoWindow = $true
            $info.RedirectStandardInput = $true
            $info.RedirectStandardOutput = $true
            $info.RedirectStandardError = $true
            $info.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
            $script:process = [Diagnostics.Process]::new()
            $script:process.StartInfo = $info
            [void]$process.Start()
            $stderr = $process.StandardError.ReadToEndAsync()
            $process.StandardInput.AutoFlush = $true
            $process.StandardInput.WriteLine((@{root=$Root; token=$token} | ConvertTo-Json -Compress))
            $ready = Read-BackendLine
            $startup = $start.Elapsed.TotalMilliseconds
            if ($mode -eq 'http') {
                $handler = [System.Net.Http.HttpClientHandler]::new()
                $handler.UseProxy = $false
                $script:http = [System.Net.Http.HttpClient]::new($handler)
                $http.Timeout = [TimeSpan]::FromSeconds(5)
                $http.DefaultRequestHeaders.Authorization = [System.Net.Http.Headers.AuthenticationHeaderValue]::new('Bearer', $token)
                $script:url = 'http://127.0.0.1:' + $ready.port
            }
            for ($i=0; $i -lt 10; $i++) { if ((Invoke-Rpc 'ping') -cne $payload) { throw 'warmup RPC mismatch' } }
            foreach ($size in @(262144, 1048576)) { Validate-Asset (Read-Asset $size) $size }
            $rpc = [System.Collections.Generic.List[double]]::new()
            for ($i=0; $i -lt 100; $i++) {
                $timer = [Diagnostics.Stopwatch]::StartNew()
                $response = Invoke-Rpc 'ping'
                $elapsed = $timer.Elapsed.TotalMilliseconds
                if ($response -cne $payload) { throw 'RPC mismatch' }
                $rpc.Add($elapsed)
            }
            $assets = @{}
            foreach ($size in @(262144, 1048576)) {
                $values = [System.Collections.Generic.List[double]]::new()
                for ($i=0; $i -lt 10; $i++) {
                    $timer = [Diagnostics.Stopwatch]::StartNew()
                    $data = Read-Asset $size
                    $elapsed = $timer.Elapsed.TotalMilliseconds
                    Validate-Asset $data $size
                    $values.Add($elapsed)
                }
                $assets[[string]$size] = @($values)
            }
            $events = [System.Collections.Generic.List[double]]::new()
            $previous = 0
            # A separate WSL command mutates the synthetic file; measured notification
            # includes that command launch and is not comparable to Linux-only polling.
            for ($i=0; $i -lt 10; $i++) {
                $timer = [Diagnostics.Stopwatch]::StartNew()
                & 'C:\Windows\System32\wsl.exe' -d $Distro --exec touch ($Root + '/changed.txt')
                if ($LASTEXITCODE -ne 0) { throw 'fixture change failed' }
                if ($mode -eq 'stdio') {
                    $event = Read-BackendLine
                } else {
                    $text = $http.GetStringAsync($url + '/events?since=' + $previous).GetAwaiter().GetResult()
                    $event = $text | ConvertFrom-Json
                }
                $elapsed = $timer.Elapsed.TotalMilliseconds
                if ($event.event -ne 'changed' -or $event.sequence -le $previous) { throw 'event mismatch' }
                $previous = $event.sequence
                $events.Add($elapsed)
            }
            $auth = $null
            if ($mode -eq 'http') {
                $publicHandler = [System.Net.Http.HttpClientHandler]::new()
                $publicHandler.UseProxy = $false
                $other = [System.Net.Http.HttpClient]::new($publicHandler)
                try {
                    $reply = $other.GetAsync($url + '/asset?size=262144').GetAwaiter().GetResult()
                    $auth = [int]$reply.StatusCode
                    $reply.Dispose()
                    if ($auth -ne 403) { throw 'unauthenticated access was not rejected' }
                } finally { $other.Dispose() }
            }
            $timer = [Diagnostics.Stopwatch]::StartNew()
            $process.StandardInput.WriteLine('{"method":"shutdown","id":-1}')
            if (-not $process.WaitForExit(5000)) { throw 'shutdown timeout' }
            $shutdown = $timer.Elapsed.TotalMilliseconds
            if ($process.ExitCode -ne 0) { throw ('backend exit ' + $process.ExitCode + ': ' + $stderr.Result) }
            $rows.Add(@{trial=$trial; mode=$mode; startup_ms=$startup; shutdown_ms=$shutdown; rpc_raw_ms=@($rpc); assets_raw_ms=$assets; notifications_including_wsl_touch_ms=@($events); received_events=$previous; unauthenticated_status=$auth; backend_python=$ready.python})
        } finally {
            if ($script:http) { $http.Dispose() }
            if ($script:process) {
                if (-not $process.HasExited) { $process.StandardInput.Close(); if (-not $process.WaitForExit(5000)) { $process.Kill(); $process.WaitForExit() } }
                $process.Dispose()
            }
        }
    }
}
$sha.Dispose()
@{trials=@($rows); powershell=$PSVersionTable.PSVersion.ToString(); scope='Windows client through wsl.exe or Windows localhost forwarding; existing running Ubuntu WSL2. No distro/VM cold boot. Synthetic fixture only.'} | ConvertTo-Json -Depth 8 -Compress
'''


def run_windows(distro):
    script = HERE / 'windows-generated.ps1'
    script.write_text(WINDOWS_SCRIPT)
    with tempfile.TemporaryDirectory(prefix='repo-notes-transport-win-') as temporary:
        root = Path(temporary)
        for size in (262144, 1048576):
            (root / f'asset-{size}.bin').write_bytes(bytes(range(256)) * (size // 256))
        (root / 'changed.txt').write_text('initial')
        script_unc = '\\\\wsl.localhost\\' + distro + str(script).replace('/', '\\')
        result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', script_unc, '-Distro', distro, '-Root', str(root), '-Backend', str(HERE / 'backend.py')], capture_output=True, timeout=180)
        (HERE / 'windows-stderr.log').write_bytes(result.stderr)
        if result.returncode:
            print(result.stderr.decode('utf-8', errors='replace'), file=sys.stderr)
            raise SystemExit(result.returncode)
        data = json.loads(result.stdout.decode('utf-8-sig'))
        data['conditions'] = {'trials': 5, 'rpc_repeats_per_trial': 100, 'asset_repeats_per_size_per_trial': 10, 'notification_repeats_per_trial': 10, 'warmup_rpc': 10, 'warmup_assets_per_size': 1, 'watch_interval_ms': 5, 'http_connection': 'persistent .NET HttpClient; proxy disabled; backend TCP_NODELAY', 'scope': data['scope'], 'distro': distro, 'timing': 'Windows PowerShell5.1/.NET client. Notifications include separate wsl.exe touch invocation, unlike Linux-only sample. Startup includes wsl.exe launch into running distro; not WSL VM cold boot. Checksums verified outside latency interval.'}
        summary = {}
        for mode in ('stdio', 'http'):
            rows = [row for row in data['trials'] if row['mode'] == mode]
            summary[mode] = {'startup': stats([r['startup_ms'] for r in rows]), 'shutdown': stats([r['shutdown_ms'] for r in rows]), 'rpc': stats([v for r in rows for v in r['rpc_raw_ms']]), 'notification_including_wsl_touch': stats([v for r in rows for v in r['notifications_including_wsl_touch_ms']]), 'assets': {str(size): stats([v for r in rows for v in r['assets_raw_ms'][str(size)]]) for size in (262144, 1048576)}, 'trial_rpc_medians_ms': [stats(r['rpc_raw_ms'])['median_ms'] for r in rows]}
        data['summary'] = summary
        (HERE / 'windows-results.json').write_text(json.dumps(data, indent=2))
        print(json.dumps({'summary': summary, 'scope': data['scope']}, indent=2))


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--windows':
        run_windows(sys.argv[2])
    else:
        run()
