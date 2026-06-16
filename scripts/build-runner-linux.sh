#!/usr/bin/env bash
# Build the static Linux duckle-runner stub used to assemble cross-OS
# "Build Pipeline" artifacts that target Linux from a non-Linux host.
#
# Why Docker: cross-compiling the runner from Windows via Zig is blocked by
# spaces in the Windows user paths (cargo-zigbuild mishandles them). Building
# inside a Linux musl container compiles natively - no cross-compile path
# issues - and yields a fully static (musl, static-pie) binary that runs on any
# Linux distro with no glibc / toolchain dependency.
#
# Output: apps/desktop/bin/duckle-runner-linux-x64
#   This is gitignored and embedded into the desktop app at `cargo tauri build`
#   time, so the shipped app can assemble a Linux bundle with NO fetched runner.
#
# Requires: Docker. Run from the repo root: bash scripts/build-runner-linux.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$REPO_ROOT/apps/desktop/bin/duckle-runner-linux-x64"
IMAGE="messense/rust-musl-cross:x86_64-musl"

echo "Building static Linux duckle-runner in $IMAGE ..."
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$REPO_ROOT:/io" -w /io \
  -e CARGO_TARGET_DIR=/tmp/t \
  "$IMAGE" \
  bash -lc "cargo build --profile release-runner --target x86_64-unknown-linux-musl -p duckle-runner \
            && cp /tmp/t/x86_64-unknown-linux-musl/release-runner/duckle-runner /io/apps/desktop/bin/duckle-runner-linux-x64"

mkdir -p "$REPO_ROOT/apps/desktop/bin"
if [ -f "$OUT" ]; then
  echo "OK -> $OUT"
  file "$OUT" 2>/dev/null || true
else
  echo "ERROR: expected $OUT was not produced" >&2
  exit 1
fi
