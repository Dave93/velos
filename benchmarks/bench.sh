#!/usr/bin/env bash
# Velos Performance Benchmark Suite
# Measures: daemon memory, startup time, IPC latency, process spawn time, binary size.
# Usage: bash benchmarks/bench.sh
# Requires: velos binary built (cargo build)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
VELOS="${PROJECT_DIR}/target/debug/velos"
BENCH_DIR="/tmp/velos-bench-$$"
SOCKET="${BENCH_DIR}/velos.sock"
STATE_DIR="${BENCH_DIR}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# Nanosecond timer (works on both macOS and Linux)
now_ns() {
    python3 -c 'import time; print(int(time.time()*1e9))'
}

cleanup() {
    # Stop daemon if running
    if [[ -f "${BENCH_DIR}/daemon.pid" ]]; then
        local pid
        pid=$(cat "${BENCH_DIR}/daemon.pid" 2>/dev/null || true)
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            sleep 0.3
            kill -9 "$pid" 2>/dev/null || true
        fi
    fi
    rm -rf "$BENCH_DIR"
}
trap cleanup EXIT

header() {
    echo -e "\n${BOLD}${CYAN}=== $1 ===${RESET}\n"
}

result() {
    printf "  %-30s %s\n" "$1:" "$2"
}

pass_fail() {
    local metric="$1" value="$2" target="$3" unit="$4"
    local val_num target_num
    val_num=$(echo "$value" | sed 's/[^0-9.]//g')
    target_num=$(echo "$target" | sed 's/[^0-9.]//g')
    if (( $(echo "$val_num <= $target_num" | bc -l 2>/dev/null || echo 0) )); then
        printf "  %-30s \033[0;32m%s%s\033[0m (target: <%s%s) \033[0;32mPASS\033[0m\n" "$metric:" "$value" "$unit" "$target" "$unit"
    else
        printf "  %-30s \033[0;31m%s%s\033[0m (target: <%s%s) \033[0;31mFAIL\033[0m\n" "$metric:" "$value" "$unit" "$target" "$unit"
    fi
}

# --- Pre-checks ---
if [[ ! -x "$VELOS" ]]; then
    # Try release build
    VELOS="${PROJECT_DIR}/target/release/velos"
    if [[ ! -x "$VELOS" ]]; then
        echo "Error: velos binary not found. Run 'cargo build' first."
        exit 1
    fi
fi

mkdir -p "$BENCH_DIR"

# Create a simple sleep script for benchmarking
SLEEP_SCRIPT="${BENCH_DIR}/sleep_forever.sh"
cat > "$SLEEP_SCRIPT" << 'SCRIPT'
#!/bin/sh
while true; do sleep 3600; done
SCRIPT
chmod +x "$SLEEP_SCRIPT"

echo -e "${BOLD}Velos Performance Benchmarks${RESET}"
echo "Binary: $VELOS"
echo "Temp dir: $BENCH_DIR"
echo "Date: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Platform: $(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')"

# Export VELOS_SOCKET so the client can find the isolated daemon
export VELOS_SOCKET="$SOCKET"

# =============================================================================
# 1. Binary Size
# =============================================================================
header "Binary Size"
BINARY_SIZE_BYTES=$(stat -f%z "$VELOS" 2>/dev/null || stat -c%s "$VELOS" 2>/dev/null)
BINARY_SIZE_MB=$(echo "scale=2; $BINARY_SIZE_BYTES / 1048576" | bc)
result "Binary size" "${BINARY_SIZE_MB} MB"

# =============================================================================
# 2. Daemon Startup Time
# =============================================================================
header "Daemon Startup Time"

START_NS=$(now_ns)
"$VELOS" daemon --socket "$SOCKET" --state-dir "$STATE_DIR" >/dev/null 2>&1 &
DAEMON_PID=$!
echo "$DAEMON_PID" > "${BENCH_DIR}/daemon.pid"

# Wait for socket to appear
WAITED=0
while [[ ! -S "$SOCKET" ]]; do
    sleep 0.01
    WAITED=$((WAITED + 1))
    if [[ $WAITED -gt 500 ]]; then
        echo "Error: daemon did not start within 5s"
        exit 1
    fi
done
END_NS=$(now_ns)

STARTUP_MS=$(echo "scale=2; ($END_NS - $START_NS) / 1000000" | bc)
result "Daemon startup" "${STARTUP_MS} ms"

sleep 0.5  # Let daemon settle

# =============================================================================
# 3. Daemon Memory Usage (idle)
# =============================================================================
header "Daemon Memory Usage (idle)"

RSS_KB=$(ps -o rss= -p "$DAEMON_PID" | tr -d ' ')
RSS_MB=$(echo "scale=2; $RSS_KB / 1024" | bc)
pass_fail "Daemon RSS (idle)" "$RSS_MB" "4" " MB"

# =============================================================================
# 4. IPC Latency (ping round-trip)
# =============================================================================
header "IPC Latency (Ping Round-Trip)"

PING_COUNT=50
TOTAL_PING_NS=0

# Warm up
"$VELOS" ping >/dev/null 2>&1 || true

for i in $(seq 1 $PING_COUNT); do
    P_START=$(now_ns)
    "$VELOS" ping >/dev/null 2>&1
    P_END=$(now_ns)
    ELAPSED=$((P_END - P_START))
    TOTAL_PING_NS=$((TOTAL_PING_NS + ELAPSED))
done

AVG_PING_MS=$(echo "scale=3; $TOTAL_PING_NS / $PING_COUNT / 1000000" | bc)
result "Avg ping latency (${PING_COUNT}x)" "${AVG_PING_MS} ms"

# =============================================================================
# 5. Process Spawn Time
# =============================================================================
header "Process Spawn Time"

SPAWN_START=$(now_ns)
"$VELOS" start "$SLEEP_SCRIPT" --name bench-proc >/dev/null 2>&1
SPAWN_END=$(now_ns)

SPAWN_MS=$(echo "scale=2; ($SPAWN_END - $SPAWN_START) / 1000000" | bc)
result "Process start (fork+exec+IPC)" "${SPAWN_MS} ms"

sleep 0.2

# =============================================================================
# 6. Daemon Memory Usage (with process)
# =============================================================================
header "Daemon Memory Usage (1 process)"

RSS_KB_1=$(ps -o rss= -p "$DAEMON_PID" | tr -d ' ')
RSS_MB_1=$(echo "scale=2; $RSS_KB_1 / 1024" | bc)
result "Daemon RSS (1 process)" "${RSS_MB_1} MB"

# =============================================================================
# 7. Multiple Process Spawn
# =============================================================================
header "Batch Process Spawn (10 processes)"

BATCH_START=$(now_ns)
for i in $(seq 1 10); do
    "$VELOS" start "$SLEEP_SCRIPT" --name "batch-$i" >/dev/null 2>&1
done
BATCH_END=$(now_ns)

BATCH_MS=$(echo "scale=2; ($BATCH_END - $BATCH_START) / 1000000" | bc)
BATCH_AVG=$(echo "scale=2; $BATCH_MS / 10" | bc)
result "10 processes total" "${BATCH_MS} ms"
result "Average per process" "${BATCH_AVG} ms"

# =============================================================================
# 8. Daemon Memory with 11 processes
# =============================================================================
header "Daemon Memory Usage (11 processes)"

sleep 0.5
RSS_KB_11=$(ps -o rss= -p "$DAEMON_PID" | tr -d ' ')
RSS_MB_11=$(echo "scale=2; $RSS_KB_11 / 1024" | bc)
result "Daemon RSS (11 processes)" "${RSS_MB_11} MB"

# =============================================================================
# 9. List Command Latency
# =============================================================================
header "List Command Latency (11 processes)"

LIST_TOTAL=0
LIST_N=20
for i in $(seq 1 $LIST_N); do
    L_START=$(now_ns)
    "$VELOS" list >/dev/null 2>&1
    L_END=$(now_ns)
    LIST_TOTAL=$((LIST_TOTAL + L_END - L_START))
done
LIST_AVG_MS=$(echo "scale=3; $LIST_TOTAL / $LIST_N / 1000000" | bc)
result "Avg list latency (${LIST_N}x)" "${LIST_AVG_MS} ms"

# =============================================================================
# Cleanup and Summary
# =============================================================================

# Stop all processes
for i in $(seq 1 10); do
    "$VELOS" stop "batch-$i" >/dev/null 2>&1 || true
done
"$VELOS" stop bench-proc >/dev/null 2>&1 || true

# Shut down daemon
kill "$DAEMON_PID" 2>/dev/null || true
sleep 0.3

header "Summary"
echo "  Binary size:           ${BINARY_SIZE_MB} MB"
echo "  Daemon startup:        ${STARTUP_MS} ms"
echo "  Daemon RSS (idle):     ${RSS_MB} MB"
echo "  Daemon RSS (11 proc):  ${RSS_MB_11} MB"
echo "  IPC ping latency:      ${AVG_PING_MS} ms (avg of ${PING_COUNT})"
echo "  Process spawn:         ${SPAWN_MS} ms"
echo "  Batch spawn (avg):     ${BATCH_AVG} ms/proc"
echo "  List latency:          ${LIST_AVG_MS} ms (avg of ${LIST_N})"
echo ""
echo -e "${GREEN}Benchmarks complete.${RESET}"
