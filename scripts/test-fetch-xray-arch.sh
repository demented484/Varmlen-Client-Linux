#!/bin/sh
set -eu

assert_asset() {
  triple="$1"
  expected="$2"
  actual="$(
    VARMLEN_TARGET_TRIPLE="$triple" \
      bash scripts/fetch-xray.sh --print-asset
  )"
  if [ "$actual" != "$expected" ]; then
    echo "$triple: expected $expected, got $actual" >&2
    exit 1
  fi
}

assert_asset x86_64-unknown-linux-gnu Xray-linux-64.zip
assert_asset i686-unknown-linux-gnu Xray-linux-32.zip
assert_asset aarch64-unknown-linux-gnu Xray-linux-arm64-v8a.zip
assert_asset armv7-unknown-linux-gnueabihf Xray-linux-arm32-v7a.zip
assert_asset arm-unknown-linux-gnueabihf Xray-linux-arm32-v6.zip
assert_asset armv5te-unknown-linux-gnueabi Xray-linux-arm32-v5.zip
assert_asset riscv64gc-unknown-linux-gnu Xray-linux-riscv64.zip
assert_asset loongarch64-unknown-linux-gnu Xray-linux-loong64.zip
assert_asset powerpc64le-unknown-linux-gnu Xray-linux-ppc64le.zip
assert_asset powerpc64-unknown-linux-gnu Xray-linux-ppc64.zip
assert_asset s390x-unknown-linux-gnu Xray-linux-s390x.zip
assert_asset mipsel-unknown-linux-gnu Xray-linux-mips32le.zip
assert_asset mips64-unknown-linux-gnuabi64 Xray-linux-mips64.zip

WORKFLOW=".github/workflows/release-linux.yml"
if [ ! -f "$WORKFLOW" ]; then
  echo "missing native Linux release workflow: $WORKFLOW" >&2
  exit 1
fi

for required in \
  "runner: ubuntu-24.04" \
  "runner: ubuntu-24.04-arm" \
  "rust_target: x86_64-unknown-linux-gnu" \
  "rust_target: aarch64-unknown-linux-gnu" \
  "asset_arch: amd64" \
  "asset_arch: arm64"; do
  if ! grep -Fq "$required" "$WORKFLOW"; then
    echo "release workflow is missing: $required" >&2
    exit 1
  fi
done

echo "Xray target architecture mapping: PASS"
