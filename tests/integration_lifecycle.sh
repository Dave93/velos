#!/usr/bin/env bash
# Integration test: full lifecycle via IPC
# daemon start → ping → start process → list → logs → stop → delete → shutdown
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
VELOS="$PROJECT_DIR/target/debug/velos"
TEST_DIR=$(mktemp -d /tmp/velos-test-XXXXXX)
SOCKET="$TEST_DIR/velos.sock"
STATE_DIR="$TEST_DIR"
DAEMON_PID=""
PASSED=0
FAILED=0
TOTAL=0

cleanup() {
    if [ -n "$DAEMON_PID" ] && kill -0 "$DAEMON_PID" 2>/dev/null; then
        kill "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

pass() { TOTAL=$((TOTAL + 1)); PASSED=$((PASSED + 1)); echo "  PASS: $1"; }
fail() { TOTAL=$((TOTAL + 1)); FAILED=$((FAILED + 1)); echo "  FAIL: $1"; [ -n "${2:-}" ] && echo "        $2"; }

echo "=== Velos Integration Test: Full Lifecycle ==="
echo "Test dir: $TEST_DIR"
echo ""

# Build if needed
[ ! -f "$VELOS" ] && (cd "$PROJECT_DIR" && make dev)

# Create the IPC helper script
cat > "$TEST_DIR/ipc_client.py" <<'PYEOF'
#!/usr/bin/env python3
"""Minimal IPC client for Velos integration tests."""
import socket, struct, sys, json, time

def write_string(s):
    b = s.encode()
    return struct.pack('<I', len(b)) + b

def read_string(data, off):
    length = struct.unpack_from('<I', data, off)[0]
    off += 4
    return data[off:off+length].decode(), off + length

def build_request(req_id, command, payload=b''):
    body = struct.pack('<I', req_id) + struct.pack('B', command) + payload
    header = b'\x56\x10\x01' + struct.pack('<I', len(body))
    return header + body

def parse_response(data):
    if len(data) < 12:  # 7 header + 4 id + 1 status
        return None
    payload_len = struct.unpack_from('<I', data, 3)[0]
    body = data[7:7+payload_len]
    req_id = struct.unpack_from('<I', body, 0)[0]
    status = body[4]
    payload = body[5:]
    return {'id': req_id, 'status': status, 'payload': payload}

def send_recv(sock_path, req_id, command, payload=b''):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(sock_path)
    s.settimeout(3)
    s.sendall(build_request(req_id, command, payload))
    resp = b''
    try:
        while True:
            chunk = s.recv(4096)
            if not chunk:
                break
            resp += chunk
            if len(resp) >= 7:
                needed = 7 + struct.unpack_from('<I', resp, 3)[0]
                if len(resp) >= needed:
                    break
    except socket.timeout:
        pass
    s.close()
    return parse_response(resp)

if __name__ == '__main__':
    sock = sys.argv[1]
    cmd = sys.argv[2]

    if cmd == 'ping':
        r = send_recv(sock, 1, 0x40)
        if r and r['status'] == 0:
            print(json.dumps({'ok': True, 'payload': r['payload'].decode()}))
        else:
            print(json.dumps({'ok': False}))

    elif cmd == 'start':
        name, script, cwd = sys.argv[3], sys.argv[4], sys.argv[5]
        payload = write_string(name) + write_string(script) + write_string(cwd)
        payload += write_string('')  # interpreter (empty = auto)
        payload += struct.pack('<I', 5000)  # kill_timeout
        payload += struct.pack('B', 0)      # autorestart=false
        r = send_recv(sock, 2, 0x01, payload)
        if r and r['status'] == 0 and len(r['payload']) >= 4:
            pid = struct.unpack_from('<I', r['payload'], 0)[0]
            print(json.dumps({'ok': True, 'process_id': pid}))
        else:
            msg = r['payload'].decode() if r else 'no response'
            print(json.dumps({'ok': False, 'error': msg}))

    elif cmd == 'list':
        r = send_recv(sock, 3, 0x05)
        if r and r['status'] == 0:
            data = r['payload']
            count = struct.unpack_from('<I', data, 0)[0]
            off = 4
            procs = []
            for _ in range(count):
                pid_id = struct.unpack_from('<I', data, off)[0]; off += 4
                name, off = read_string(data, off)
                pid = struct.unpack_from('<I', data, off)[0]; off += 4
                status = data[off]; off += 1
                mem = struct.unpack_from('<Q', data, off)[0]; off += 8
                uptime = struct.unpack_from('<Q', data, off)[0]; off += 8
                restarts = struct.unpack_from('<I', data, off)[0]; off += 4
                procs.append({'id': pid_id, 'name': name, 'pid': pid, 'status': status})
            print(json.dumps({'ok': True, 'count': count, 'processes': procs}))
        else:
            print(json.dumps({'ok': False}))

    elif cmd == 'logs':
        proc_id, lines = int(sys.argv[3]), int(sys.argv[4])
        payload = struct.pack('<II', proc_id, lines)
        r = send_recv(sock, 4, 0x10, payload)
        if r and r['status'] == 0:
            data = r['payload']
            count = struct.unpack_from('<I', data, 0)[0]
            off = 4
            entries = []
            for _ in range(count):
                ts = struct.unpack_from('<Q', data, off)[0]; off += 8
                level = data[off]; off += 1
                stream = data[off]; off += 1
                msg, off = read_string(data, off)
                entries.append({'message': msg, 'stream': stream})
            print(json.dumps({'ok': True, 'count': count, 'entries': entries}))
        else:
            print(json.dumps({'ok': False}))

    elif cmd == 'stop':
        proc_id = int(sys.argv[3])
        payload = struct.pack('<I', proc_id) + struct.pack('B', 15) + struct.pack('<I', 5000)
        r = send_recv(sock, 5, 0x02, payload)
        print(json.dumps({'ok': r and r['status'] == 0}))

    elif cmd == 'delete':
        proc_id = int(sys.argv[3])
        payload = struct.pack('<I', proc_id)
        r = send_recv(sock, 6, 0x04, payload)
        print(json.dumps({'ok': r and r['status'] == 0}))

    elif cmd == 'shutdown':
        r = send_recv(sock, 7, 0x41)
        print(json.dumps({'ok': r and r['status'] == 0}))
PYEOF

IPC="python3 $TEST_DIR/ipc_client.py $SOCKET"

# Create test script
cat > "$TEST_DIR/hello.sh" <<'SCRIPT'
#!/bin/sh
echo "hello from velos"
echo "line two output"
sleep 60
SCRIPT
chmod +x "$TEST_DIR/hello.sh"

# --- 1. Start daemon ---
echo "1. Start daemon"
"$VELOS" daemon --socket "$SOCKET" --state-dir "$STATE_DIR" >/dev/null 2>&1 &
DAEMON_PID=$!
for i in $(seq 1 20); do [ -S "$SOCKET" ] && break; sleep 0.1; done
if [ -S "$SOCKET" ]; then pass "daemon started, socket ready"
else fail "socket not found"; exit 1; fi
echo ""

# --- 2. Ping ---
echo "2. Ping"
RESULT=$($IPC ping)
if echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); exit(0 if d['ok'] and 'pong' in d.get('payload','') else 1)" 2>/dev/null; then
    pass "ping → pong"
else
    fail "ping failed" "$RESULT"
fi
echo ""

# --- 3. Start process ---
echo "3. Start process"
RESULT=$($IPC start "test-app" "$TEST_DIR/hello.sh" "$TEST_DIR")
PROC_ID=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('process_id',0))" 2>/dev/null || echo 0)
if [ "$PROC_ID" -gt 0 ] 2>/dev/null; then
    pass "started process id=$PROC_ID"
else
    fail "start failed" "$RESULT"
fi
sleep 0.5  # let process produce output
echo ""

# --- 4. List processes ---
echo "4. List processes"
RESULT=$($IPC list)
if echo "$RESULT" | python3 -c "
import sys,json
d=json.load(sys.stdin)
assert d['ok'] and d['count'] > 0
assert any(p['name']=='test-app' for p in d['processes'])
" 2>/dev/null; then
    pass "list contains 'test-app'"
else
    fail "list failed" "$RESULT"
fi
echo ""

# --- 5. Read logs ---
echo "5. Read logs"
RESULT=$($IPC logs "$PROC_ID" 10)
if echo "$RESULT" | python3 -c "
import sys,json
d=json.load(sys.stdin)
assert d['ok']
if d['count'] > 0:
    assert any('hello' in e['message'] for e in d['entries'])
" 2>/dev/null; then
    LOG_COUNT=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['count'])" 2>/dev/null)
    pass "logs returned $LOG_COUNT entries"
else
    fail "logs failed" "$RESULT"
fi
echo ""

# --- 6. Stop process ---
echo "6. Stop process"
RESULT=$($IPC stop "$PROC_ID")
if echo "$RESULT" | python3 -c "import sys,json; assert json.load(sys.stdin)['ok']" 2>/dev/null; then
    pass "process stopped"
else
    fail "stop failed" "$RESULT"
fi
sleep 0.3
echo ""

# --- 7. Delete process ---
echo "7. Delete process"
RESULT=$($IPC delete "$PROC_ID")
if echo "$RESULT" | python3 -c "import sys,json; assert json.load(sys.stdin)['ok']" 2>/dev/null; then
    pass "process deleted"
else
    fail "delete failed" "$RESULT"
fi
echo ""

# --- 8. Verify empty list ---
echo "8. Verify empty list"
RESULT=$($IPC list)
if echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['ok'] and d['count']==0" 2>/dev/null; then
    pass "list is empty after delete"
else
    fail "list not empty" "$RESULT"
fi
echo ""

# --- 9. Shutdown daemon ---
echo "9. Shutdown daemon"
kill "$DAEMON_PID" 2>/dev/null
wait "$DAEMON_PID" 2>/dev/null || true
DAEMON_PID=""

if [ ! -S "$SOCKET" ]; then pass "socket cleaned up"
else fail "socket still exists"; fi
echo ""

# --- Results ---
echo "=== Results: $PASSED/$TOTAL passed ==="
if [ "$FAILED" -gt 0 ]; then
    echo "$FAILED test(s) FAILED"
    exit 1
else
    echo "All tests passed!"
fi
