#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${1:-/tmp/loom-rehearsal}"
KERNEL_PATH="${KERNEL_PATH:-/tmp/meridian-kernel}"

echo "== Meridian Loom rehearsal =="
echo "root:   ${ROOT_DIR}"
echo "kernel: ${KERNEL_PATH}"

rm -rf "${ROOT_DIR}"

cargo test --workspace
cargo build --workspace

./target/debug/loom init --mode embedded --kernel-path "${KERNEL_PATH}" --root "${ROOT_DIR}" --org-id rehearsal_org
./target/debug/loom doctor --root "${ROOT_DIR}" --format human
./target/debug/loom health --root "${ROOT_DIR}" --format json
./target/debug/loom status --root "${ROOT_DIR}"
./target/debug/loom contract show --root "${ROOT_DIR}"
./target/debug/loom capsule inspect --root "${ROOT_DIR}"
./target/debug/loom shadow report --root "${ROOT_DIR}"

echo "== Rehearsal complete =="
