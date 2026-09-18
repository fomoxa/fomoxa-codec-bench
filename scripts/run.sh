#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="$repo_root/target/release"

millis=500
repeats=5
seconds=3
messages=""
formats=""
modes=""
out="${TMPDIR:-/tmp}/fomoxa-codec-bench/results.md"

usage() {
    cat >&2 <<EOF
usage: scripts/run.sh [--millis=500] [--repeats=5] [--seconds=3]
                      [--messages=input,transform,batch-10,batch-100,batch-1000,chat,stats]
                      [--formats=fomoxa,protobuf,capnp,capnp-canonical]
                      [--modes=buffered,unbuffered,echo] [--out=<file>]

  1. codec: size, encode and decode time of every sample in every format
     (each cell is the median of --repeats runs of --millis)
  2. tcp:   the same samples sent over loopback TCP, --seconds per cell:
            buffered and unbuffered one-way streams, and request/response echo

  both tables are printed and written to --out (default $out)
EOF
    exit 2
}

for argument in "$@"; do
    case "$argument" in
        --millis=*) millis="${argument#*=}" ;;
        --repeats=*) repeats="${argument#*=}" ;;
        --seconds=*) seconds="${argument#*=}" ;;
        --messages=*) messages="${argument#*=}" ;;
        --formats=*) formats="${argument#*=}" ;;
        --modes=*) modes="${argument#*=}" ;;
        --out=*) out="${argument#*=}" ;;
        -h | --help) usage ;;
        *)
            echo "scripts/run.sh: unknown argument $argument" >&2
            usage
            ;;
    esac
done

cargo build --release --quiet --manifest-path "$repo_root/Cargo.toml"
mkdir -p "$(dirname "$out")"

selection=()
[[ -n "$messages" ]] && selection+=(--messages "$messages")
[[ -n "$formats" ]] && selection+=(--formats "$formats")
mode_selection=()
[[ -n "$modes" ]] && mode_selection+=(--modes "$modes")

{
    echo "# fomoxa-codec-bench $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo
    echo "$(uname -sr) · $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //') · $(nproc) logical CPUs · $(rustc --version)"
    echo
    "$bin/codec" --millis "$millis" --repeats "$repeats" "${selection[@]}"
    echo
    "$bin/tcp" --seconds "$seconds" "${selection[@]}" "${mode_selection[@]}"
} | tee "$out"

echo >&2
echo "results written to $out" >&2
