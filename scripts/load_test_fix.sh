#!/usr/bin/env bash
# Simple FIX TCP load tester
# Usage:
#   ./load_test_fix.sh -h 127.0.0.1 -p 8888 -c 10 -r 100 -i 0.1
# Options:
#   -h host (default 127.0.0.1)
#   -p port (default 8888)
#   -c concurrency (number of parallel workers, default 10)
#   -r requests per worker (default 100)
#   -i interval seconds between requests per worker (default 0.1)
#   -m message-file: path to a file containing raw FIX message to send; if omitted a default message is used.
# Notes: requires `nc` (netcat). If nc absent, it will try bash /dev/tcp fallback.

set -euo pipefail

HOST="127.0.0.1"
PORT=8888
CONCURRENCY=10
REQUESTS_PER_WORKER=100
INTERVAL=0.1
MSG_FILE=""

while getopts ":h:p:c:r:i:m:" opt; do
  case ${opt} in
    h ) HOST=$OPTARG ;;
    p ) PORT=$OPTARG ;;
    c ) CONCURRENCY=$OPTARG ;;
    r ) REQUESTS_PER_WORKER=$OPTARG ;;
    i ) INTERVAL=$OPTARG ;;
    m ) MSG_FILE=$OPTARG ;;
    \? ) echo "Usage: $0 [-h host] [-p port] [-c concurrency] [-r requests_per_worker] [-i interval] [-m message_file]"; exit 1 ;;
  esac
done

if [[ -n "$MSG_FILE" && -f "$MSG_FILE" ]]; then
  FIX_MSG=$(cat "$MSG_FILE")
else
  # Default pipe-delimited test message (server accepts | or SOH as fallback)
  FIX_MSG='8=FIX.4.2|9=...|35=D|55=Pranesh|54=1|44=100|38=10|10=000|\n'
fi

echo "Load test: host=$HOST port=$PORT concurrency=$CONCURRENCY requests_per_worker=$REQUESTS_PER_WORKER interval=$INTERVAL"

# Worker function
worker() {
  local id=$1
  local sent=0
  for ((i=1;i<=REQUESTS_PER_WORKER;i++)); do
    # send message and capture response (if any)
    if command -v nc >/dev/null 2>&1; then
      # -w 2 sets a short timeout for connect+io for most netcat versions
      # -N closes the socket after EOF on some nc implementations; -q 1 is alternate
      resp=$(printf "%s" "$FIX_MSG" | nc -w 2 "$HOST" "$PORT" 2>/dev/null || true)
    else
      # bash /dev/tcp fallback (may not be available on all systems)
      exec 3>/dev/tcp/${HOST}/${PORT} 2>/dev/null || true
      if [[ -e /proc/$$/fd/3 ]]; then
        printf "%s" "$FIX_MSG" >&3 || true
        # try to read response with timeout
        resp=""
        IFS= read -t 1 -r resp <&3 || true
        exec 3>&-
      else
        resp=""
      fi
    fi

    if [[ -n "$resp" ]]; then
      printf "%s\n" "[worker $id] ok: got ${#resp} bytes"
    else
      printf "%s\n" "[worker $id] ok: no response"
    fi

    sent=$((sent+1))
    # sleep if not last
    if [[ $i -lt $REQUESTS_PER_WORKER ]]; then
      sleep $INTERVAL
    fi
  done
  printf "[worker %s] finished sent=%s\n" "$id" "$sent"
}

# Start workers
pids=()
for ((w=1; w<=CONCURRENCY; w++)); do
  worker $w &
  pids+=("$!")
done

# Wait for all workers
for pid in "${pids[@]}"; do
  wait "$pid" || true
done

echo "All workers finished."