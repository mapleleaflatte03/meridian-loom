#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

required_files=(
  "Cargo.toml"
  "README.md"
  "LICENSE"
  "loom.toml.example"
  ".github/workflows/rust.yml"
  "docs/SETUP_REHEARSAL.md"
  "docs/PUBLICATION_CHECKLIST.md"
  "examples/rehearsal-output.txt"
)

echo "== Meridian Loom publication readiness =="
echo "repo: $ROOT"

for path in "${required_files[@]}"; do
  if [[ -f "$path" ]]; then
    echo "[OK]   $path"
  else
    echo "[FAIL] missing $path"
    exit 1
  fi
done

echo
echo "== cargo test =="
cargo test --workspace

echo
echo "== cargo build =="
cargo build --workspace

echo
echo "== setup rehearsal =="
./scripts/rehearse_setup.sh >/tmp/meridian-loom-publication-rehearsal.txt
echo "[OK]   rehearsal transcript refreshed at /tmp/meridian-loom-publication-rehearsal.txt"

echo
echo "== git status =="
git status --short

echo
echo "== gh auth status =="
if gh auth status >/tmp/meridian-loom-gh-auth.txt 2>&1; then
  cat /tmp/meridian-loom-gh-auth.txt
else
  cat /tmp/meridian-loom-gh-auth.txt
  echo
  echo "[WARN] GitHub auth is not ready. Public publication remains blocked."
fi
