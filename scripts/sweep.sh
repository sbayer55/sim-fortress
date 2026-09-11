#!/usr/bin/env bash
# Sweep seeds headless and write summary.csv (C6 FR8).
#
#   scripts/sweep.sh <first> <last> <years>
#
# Each seed runs as a separate `sim-fortress --headless --seed N --years Y --row`
# invocation in parallel (xargs -P $(nproc)); the single-row outputs are sorted
# by seed and assembled into summary.csv with a header.

set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <first-seed> <last-seed> <years>" >&2
    exit 2
fi

first="$1"
last="$2"
years="$3"

here="$(cd "$(dirname "$0")/.." && pwd)"
bin="${SIM_FORTRESS_BIN:-$here/target/release/sim-fortress}"

# Build once if the binary is missing.
if [ ! -x "$bin" ]; then
    (cd "$here" && cargo build --release)
fi

if command -v nproc >/dev/null 2>&1; then
    jobs="$(nproc)"
elif command -v getconf >/dev/null 2>&1 && getconf _NPROCESSORS_ONLN >/dev/null 2>&1; then
    jobs="$(getconf _NPROCESSORS_ONLN)"
else
    jobs=4
fi

header_file="$(mktemp)"
sorted_file="$(mktemp)"
trap 'rm -f "$header_file" "$sorted_file"' EXIT

"$bin" --header > "$header_file"

# One row per seed, in parallel.
seq "$first" "$last" \
    | xargs -P "$jobs" -I{} sh -c '"$0" --headless --seed "$1" --years "$2" --row' "$bin" {} "$years" \
    > "$sorted_file"

# Assemble header + data rows sorted numerically by seed.
{
    cat "$header_file"
    sort -t, -k1,1n "$sorted_file"
} > summary.csv

cat summary.csv
echo "summary.csv written: $(( $(wc -l < summary.csv) - 1 )) seeds" >&2
