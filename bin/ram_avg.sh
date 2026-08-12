#!/bin/bash
# usage: ./ram_avg.sh <process_name> [duration_seconds]

set -u

PROC="${1:-}"
DURATION="${2:-300}"
INTERVAL=10

if [ -z "$PROC" ]; then
  echo "usage: $0 <process_name> [duration_seconds]" >&2
  exit 1
fi

kb_to_mb() {
  awk -v kb="$1" 'BEGIN { printf "%.1f", kb / 1024 }'
}

pids_by_command() {
  local pgid
  pgid=$(ps -o pgid= -p $$ | tr -d ' ')
  ps -Ao pid=,pgid=,command= | awk -v want="$PROC" -v mine="$pgid" '
    $2 != mine && index(tolower($3), tolower(want)) { print $1 }
  '
}

pids_by_display_name() {
  command -v lsappinfo >/dev/null 2>&1 || return 0
  lsappinfo list 2>/dev/null | awk -v want="$PROC" '
    /^ *[0-9]+\) "/ {
      name = $0
      sub(/^[^"]*"/, "", name)
      sub(/".*$/, "", name)
      wanted = index(tolower(name), tolower(want))
      next
    }
    wanted && match($0, /pid = [0-9]+/) {
      print substr($0, RSTART + 6, RLENGTH - 6)
      wanted = 0
    }
  '
}

total_footprint_kb() {
  local pids args
  pids=$({ pids_by_command; pids_by_display_name; } | sort -un)
  [ -n "$pids" ] || return 1

  args=$(printf -- '-pid %s ' $pids)
  top -l 1 $args -stats pid,mem 2>/dev/null | awk '
    $1 ~ /^[0-9]+$/ && $2 ~ /^[0-9.]+[BKMG]?$/ {
      size = $2 + 0
      unit = substr($2, length($2))
      if (unit == "B") size /= 1024
      else if (unit == "M") size *= 1024
      else if (unit == "G") size *= 1048576
      total += size
    }
    END { printf "%d\n", total }
  '
}

SAMPLES=$((DURATION / INTERVAL))
[ "$SAMPLES" -ge 1 ] || SAMPLES=1

echo "Sampling '$PROC' every ${INTERVAL}s for ${DURATION}s (${SAMPLES} samples)..."

total_kb=0
taken=0

for i in $(seq 1 "$SAMPLES"); do
  sample_kb=$(total_footprint_kb) || sample_kb=0

  if [ "$sample_kb" -gt 0 ]; then
    total_kb=$((total_kb + sample_kb))
    taken=$((taken + 1))
    echo "[$i/$SAMPLES] $(kb_to_mb "$sample_kb") MB"
  else
    echo "[$i/$SAMPLES] process not found, skipping"
  fi

  if [ "$i" -lt "$SAMPLES" ]; then
    sleep "$INTERVAL"
  fi
done

if [ "$taken" -eq 0 ]; then
  echo "No samples collected, is the process name right?" >&2
  exit 1
fi

echo
echo "Average over $taken samples: $(kb_to_mb $((total_kb / taken))) MB"