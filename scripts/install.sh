#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
PREFIX="${LOOM_PREFIX:-/root/.local/share/meridian-loom}"
BIN_DIR="${LOOM_BIN_DIR:-${PREFIX}/current/bin}"
RUNTIME_ROOT="${LOOM_RUNTIME_ROOT:-${PREFIX}/runtime/default}"
BINARY_PATH="${BIN_DIR}/loom"
KERNEL_PATH="${LOOM_KERNEL_PATH:-}"

SUDO=()

ensure_privileges() {
  local prefix_parent
  prefix_parent="$(dirname "$PREFIX")"
  if [[ -w "$prefix_parent" || -w "$PREFIX" ]]; then
    return
  fi
  if command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
    SUDO=(sudo -n)
    return
  fi
  echo "cannot write to $PREFIX and passwordless sudo is unavailable" >&2
  exit 1
}

run_privileged() {
  if [[ ${#SUDO[@]} -gt 0 ]]; then
    "${SUDO[@]}" "$@"
  else
    "$@"
  fi
}

file_exists() {
  if [[ ${#SUDO[@]} -gt 0 ]]; then
    "${SUDO[@]}" test -f "$1"
  else
    [[ -f "$1" ]]
  fi
}

print_banner() {
  local icon
  icon="$(cat <<'BANNER'
      /\/\
   .-/ /\ \-.
  /__/_/\_\__\
  \  \ \/ /  /
   '.__\__/_.
BANNER
)"
  if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    printf '\033[1;92m%s\033[0m\n' "$icon"
    printf '\033[1;96m%s\033[0m\n' 'MERIDIAN LOOM'
    printf '\033[96m%s\033[0m\n' 'Constitutional Runtime v0.1.0.'
    printf '\033[37m%s\033[0m\n\n' 'Autonomous intelligence inside a governed shell.'
  else
    printf '%s\n' "$icon"
    printf '%s\n' 'MERIDIAN LOOM'
    printf '%s\n' 'Constitutional Runtime v0.1.0.'
    printf '%s\n\n' 'Autonomous intelligence inside a governed shell.'
  fi
}

ensure_cargo() {
  if [[ -f /root/.cargo/env ]]; then
    # shellcheck disable=SC1091
    source /root/.cargo/env
  fi
  if [[ -f /home/ubuntu/.cargo/env ]]; then
    # shellcheck disable=SC1091
    source /home/ubuntu/.cargo/env
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    echo 'cargo not found after loading rustup environment' >&2
    exit 1
  fi
}

build_release() {
  printf '==> Building Meridian Loom release\n'
  (
    cd "$REPO_ROOT"
    cargo build --release -p meridian-loom
  )
}

install_binary() {
  local source_path
  source_path="$REPO_ROOT/target/release/loom"
  if [[ ! -x "$source_path" ]]; then
    echo "missing release binary: $source_path" >&2
    exit 1
  fi
  run_privileged mkdir -p "$BIN_DIR"
  run_privileged install -m 0755 "$source_path" "$BINARY_PATH"
  printf '==> Installed loom binary to %s\n' "$BINARY_PATH"
}

ensure_runtime_root() {
  run_privileged mkdir -p "$RUNTIME_ROOT/capabilities"
  if [[ -z "$KERNEL_PATH" && -d /tmp/meridian-kernel ]]; then
    KERNEL_PATH=/tmp/meridian-kernel
  fi

  if ! file_exists "$RUNTIME_ROOT/loom.toml"; then
    printf '==> Initializing runtime root at %s\n' "$RUNTIME_ROOT"
    if [[ -n "$KERNEL_PATH" ]]; then
      run_privileged "$BINARY_PATH" init --mode embedded --root "$RUNTIME_ROOT" --kernel-path "$KERNEL_PATH"
    else
      run_privileged "$BINARY_PATH" init --mode embedded --root "$RUNTIME_ROOT"
    fi
  else
    printf '==> Reusing existing runtime config at %s\n' "$RUNTIME_ROOT/loom.toml"
  fi

  if ! file_exists "$RUNTIME_ROOT/capabilities/registry.json"; then
    printf '==> Scaffolding capability registry\n'
    run_privileged "$BINARY_PATH" capability list --root "$RUNTIME_ROOT" --format json >/dev/null
  else
    printf '==> Reusing existing capability registry at %s\n' "$RUNTIME_ROOT/capabilities/registry.json"
  fi
}

print_summary() {
  cat <<SUMMARY
==> Installation complete
binary:   $BINARY_PATH
runtime:  $RUNTIME_ROOT
config:   $RUNTIME_ROOT/loom.toml
registry: $RUNTIME_ROOT/capabilities/registry.json
next:     $BINARY_PATH doctor --root "$RUNTIME_ROOT" --format human
SUMMARY
}

main() {
  print_banner
  ensure_cargo
  ensure_privileges
  build_release
  install_binary
  ensure_runtime_root
  print_summary
}

main "$@"
