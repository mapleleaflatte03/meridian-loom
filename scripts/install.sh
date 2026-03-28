#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
HOME_DIR="${HOME:-$(getent passwd "$(id -u)" | cut -d: -f6)}"
if [[ -z "$HOME_DIR" ]]; then
  echo 'failed to resolve HOME directory for installer prefix' >&2
  exit 1
fi
export HOME="$HOME_DIR"

PREFIX="${LOOM_PREFIX:-${HOME_DIR}/.local/share/meridian-loom}"
BIN_DIR="${LOOM_BIN_DIR:-${PREFIX}/current/bin}"
RUNTIME_ROOT="${LOOM_RUNTIME_ROOT:-${PREFIX}/runtime/default}"
BINARY_PATH="${BIN_DIR}/loom"
KERNEL_PATH="${LOOM_KERNEL_PATH:-}"
CARGO_HOME="${CARGO_HOME:-${HOME_DIR}/.cargo}"
RUSTUP_HOME="${RUSTUP_HOME:-${HOME_DIR}/.rustup}"
CARGO_ENV="${CARGO_HOME}/env"
RUSTUP_INIT_URL="https://sh.rustup.rs"

SUDO=()
APT_UPDATED=0
RUNTIME_WAS_INITIALIZED=0
ONBOARD_WAS_RUN=0

ensure_admin_access() {
  local reason="${1:-administrative action}"
  if [[ $(id -u) -eq 0 ]]; then
    return
  fi
  if [[ ${#SUDO[@]} -gt 0 ]]; then
    return
  fi
  if command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
    SUDO=(sudo -n)
    return
  fi
  printf 'administrative privileges are required to %s\n' "$reason" >&2
  exit 1
}

ensure_privileges() {
  local writable_probe
  writable_probe="$PREFIX"
  while [[ ! -e "$writable_probe" && "$writable_probe" != "/" ]]; do
    writable_probe="$(dirname "$writable_probe")"
  done
  if [[ -w "$writable_probe" || -w "$PREFIX" ]]; then
    return
  fi
  ensure_admin_access "write to $PREFIX"
}

run_privileged() {
  if [[ $(id -u) -eq 0 ]]; then
    "$@"
  elif [[ ${#SUDO[@]} -gt 0 ]]; then
    "${SUDO[@]}" "$@"
  else
    "$@"
  fi
}

run_admin() {
  if [[ $(id -u) -eq 0 ]]; then
    "$@"
  elif [[ ${#SUDO[@]} -gt 0 ]]; then
    "${SUDO[@]}" "$@"
  else
    printf 'administrative privileges are required to run %s\n' "$1" >&2
    exit 1
  fi
}

file_exists() {
  if [[ ${#SUDO[@]} -gt 0 ]]; then
    "${SUDO[@]}" test -f "$1"
  else
    [[ -f "$1" ]]
  fi
}

ensure_apt_updated() {
  if [[ "$APT_UPDATED" -eq 1 ]]; then
    return
  fi
  if ! command -v apt-get >/dev/null 2>&1; then
    echo 'apt-get is required to install missing dependencies automatically' >&2
    exit 1
  fi
  ensure_admin_access 'install missing system packages'
  printf '==> Refreshing apt metadata\n'
  run_admin env DEBIAN_FRONTEND=noninteractive apt-get -qq update
  APT_UPDATED=1
}

ensure_command_or_package() {
  local command_name="$1"
  local package_name="$2"
  if command -v "$command_name" >/dev/null 2>&1; then
    return
  fi
  ensure_apt_updated
  printf '==> Installing missing package %s for %s\n' "$package_name" "$command_name"
  run_admin env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends "$package_name"
}

seed_builtin_capabilities() {
  local registry_path
  registry_path="$RUNTIME_ROOT/capabilities/registry.json"
  run_privileged python3 - "$registry_path" <<'PY2'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
if path.exists():
    payload = json.loads(path.read_text(encoding="utf-8"))
else:
    payload = {"version": "loom.capabilities.v0", "capabilities": []}

descriptors = [
    {
        "name": "loom.browser.navigate.v1",
        "description": "Run the built-in bounded browser navigation Wasm guest through the local Wasmtime lane.",
        "action_type": "browse",
        "resource": "capability:loom.browser.navigate.v1",
        "worker_kind": "wasm",
        "interpreter": "wasmtime",
        "worker_entry": "",
        "wasm_module": "builtin:browser.navigate",
        "binary_surface": "builtin_wasm_guest",
        "payload_mode": "json",
        "source_kind": "loom_builtin",
        "source_path": "builtin:loom.browser.navigate.v1",
        "source_manifest": "",
        "adapter_kind": "loom_wasm_browser_guest_v0",
        "import_provenance": "loom_builtin_contract_v0",
        "runtime_lane": "wasm",
        "dependency_mode": "builtin",
        "env_contract": "none",
        "isolation_expectation": "pooled_wasmtime_local",
        "verification_status": "builtin",
        "last_verified_at": "",
        "last_verification_job_id": "",
        "last_verification_execution_id": "",
        "verification_note": "built-in bounded web/Wasm capability",
        "promotion_state": "builtin",
        "promoted_at": "",
        "enabled": True,
    },
    {
        "name": "loom.terminal.exec.v1",
        "description": "Run the built-in bounded terminal execution Wasm guest through the local Wasmtime lane.",
        "action_type": "execute",
        "resource": "capability:loom.terminal.exec.v1",
        "worker_kind": "wasm",
        "interpreter": "wasmtime",
        "worker_entry": "",
        "wasm_module": "builtin:terminal.exec",
        "binary_surface": "builtin_wasm_guest",
        "payload_mode": "json",
        "source_kind": "loom_builtin",
        "source_path": "builtin:loom.terminal.exec.v1",
        "source_manifest": "",
        "adapter_kind": "loom_wasm_terminal_guest_v0",
        "import_provenance": "loom_builtin_contract_v0",
        "runtime_lane": "wasm",
        "dependency_mode": "builtin",
        "env_contract": "none",
        "isolation_expectation": "pooled_wasmtime_local",
        "verification_status": "builtin",
        "last_verified_at": "",
        "last_verification_job_id": "",
        "last_verification_execution_id": "",
        "verification_note": "built-in bounded terminal/Wasm capability",
        "promotion_state": "builtin",
        "promoted_at": "",
        "enabled": True,
    },
    {
        "name": "loom.heartbeat.schedule.v1",
        "description": "Run the built-in heartbeat scheduling Wasm guest through the local Wasmtime lane and persist a bounded receipt.",
        "action_type": "schedule",
        "resource": "capability:loom.heartbeat.schedule.v1",
        "worker_kind": "wasm",
        "interpreter": "wasmtime",
        "worker_entry": "",
        "wasm_module": "builtin:heartbeat.schedule",
        "binary_surface": "builtin_wasm_guest",
        "payload_mode": "json",
        "source_kind": "loom_builtin",
        "source_path": "builtin:loom.heartbeat.schedule.v1",
        "source_manifest": "",
        "adapter_kind": "loom_wasm_heartbeat_guest_v0",
        "import_provenance": "loom_builtin_contract_v0",
        "runtime_lane": "wasm",
        "dependency_mode": "builtin",
        "env_contract": "none",
        "isolation_expectation": "pooled_wasmtime_local",
        "verification_status": "builtin",
        "last_verified_at": "",
        "last_verification_job_id": "",
        "last_verification_execution_id": "",
        "verification_note": "built-in heartbeat/Wasm capability with local receipt logging",
        "promotion_state": "builtin",
        "promoted_at": "",
        "enabled": True,
    },
]

capabilities = payload.setdefault("capabilities", [])
by_name = {item.get("name"): index for index, item in enumerate(capabilities)}
for descriptor in descriptors:
    index = by_name.get(descriptor["name"])
    if index is None:
        capabilities.append(descriptor)
    else:
        capabilities[index] = descriptor
capabilities.sort(key=lambda item: item.get("name", ""))
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY2
  printf '==> Seeded built-in Wasm capabilities into %s\n' "$registry_path"
}

print_banner() {
  local icon
  icon="$(cat <<'BANNER'
      /\      /\
     /  \    /  \
    / /\ \  / /\ \
   / /  \ \/ /  \ \
  /_/    \__/    \_\
  \ \    /  \    / / /
   \ \  / /\ \  / / /
    \_\/_/  \_\/_/_/
BANNER
)"
  if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    printf '[1;92m%s[0m
' "$icon"
    printf '[1;96m%s[0m
' 'MERIDIAN LOOM'
    printf '[37m%s[0m

' 'A governed agent fabric for bounded autonomous work.'
  else
    printf '%s
' "$icon"
    printf '%s
' 'MERIDIAN LOOM'
    printf '%s

' 'A governed agent fabric for bounded autonomous work.'
  fi
}

source_cargo_env() {
  if [[ -f "$CARGO_ENV" ]]; then
    # shellcheck disable=SC1090
    source "$CARGO_ENV"
  fi
}

bootstrap_rust_toolchain() {
  ensure_command_or_package curl curl
  ensure_command_or_package cc build-essential
  printf '==> Installing Rust toolchain via rustup\n'
  export CARGO_HOME RUSTUP_HOME HOME
  curl --proto '=https' --tlsv1.2 -fsSL "$RUSTUP_INIT_URL" | sh -s -- -y --profile minimal --default-toolchain stable
}

ensure_cargo() {
  source_cargo_env
  if command -v cargo >/dev/null 2>&1; then
    return
  fi
  bootstrap_rust_toolchain
  source_cargo_env
  if ! command -v cargo >/dev/null 2>&1; then
    echo 'cargo not found after rustup bootstrap' >&2
    exit 1
  fi
}

ensure_python3() {
  ensure_command_or_package python3 python3
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
    RUNTIME_WAS_INITIALIZED=1
  else
    printf '==> Reusing existing runtime config at %s\n' "$RUNTIME_ROOT/loom.toml"
  fi

  if ! file_exists "$RUNTIME_ROOT/capabilities/registry.json"; then
    printf '==> Scaffolding capability registry\n'
    run_privileged "$BINARY_PATH" capability list --root "$RUNTIME_ROOT" --format json >/dev/null
  else
    printf '==> Reusing existing capability registry at %s\n' "$RUNTIME_ROOT/capabilities/registry.json"
  fi

  ensure_python3
  seed_builtin_capabilities
}

run_onboard_if_interactive() {
  if [[ "${LOOM_SKIP_ONBOARD:-}" == "1" ]]; then
    return
  fi
  if [[ "$RUNTIME_WAS_INITIALIZED" -ne 1 && "${LOOM_RUN_ONBOARD:-}" != "1" ]]; then
    return
  fi
  if [[ ! -e /dev/tty || ! -r /dev/tty || ! -w /dev/tty ]]; then
    return
  fi
  if [[ ! -t 1 && ! -t 2 ]]; then
    return
  fi
  printf '==> Launching Meridian setup wizard\n'
  local onboard_args=(onboard --root "$RUNTIME_ROOT" --format human)
  if [[ -n "$KERNEL_PATH" ]]; then
    onboard_args+=(--kernel-path "$KERNEL_PATH")
  fi
  "$BINARY_PATH" "${onboard_args[@]}" </dev/tty >/dev/tty 2>/dev/tty
  ONBOARD_WAS_RUN=1
}

print_summary() {
  local next_step
  if [[ "$ONBOARD_WAS_RUN" -eq 1 ]]; then
    next_step="$BINARY_PATH doctor --root \"$RUNTIME_ROOT\" --format human"
  elif [[ "$RUNTIME_WAS_INITIALIZED" -eq 1 ]]; then
    next_step="$BINARY_PATH onboard --root \"$RUNTIME_ROOT\" --format human"
  else
    next_step="$BINARY_PATH doctor --root \"$RUNTIME_ROOT\" --format human"
  fi
  cat <<SUMMARY
==> Installation complete
binary:   $BINARY_PATH
runtime:  $RUNTIME_ROOT
config:   $RUNTIME_ROOT/loom.toml
registry: $RUNTIME_ROOT/capabilities/registry.json
next:     $next_step
SUMMARY
}

main() {
  print_banner
  ensure_privileges
  ensure_cargo
  build_release
  install_binary
  ensure_runtime_root
  run_onboard_if_interactive
  print_summary
}

main "$@"
