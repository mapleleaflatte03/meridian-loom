#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
PREFIX="${LOOM_PREFIX:-/home/ubuntu/.local/share/meridian-loom}"
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

seed_builtin_browser_capability() {
  local registry_path
  registry_path="$RUNTIME_ROOT/capabilities/registry.json"
  run_privileged python3 - "$registry_path" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
if path.exists():
    payload = json.loads(path.read_text(encoding="utf-8"))
else:
    payload = {"version": "loom.capabilities.v0", "capabilities": []}

descriptor = {
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
}

capabilities = payload.setdefault("capabilities", [])
for index, capability in enumerate(capabilities):
    if capability.get("name") == descriptor["name"]:
        capabilities[index] = descriptor
        break
else:
    capabilities.append(descriptor)
capabilities.sort(key=lambda item: item.get("name", ""))
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
  printf "==> Seeded built-in browser capability into %s\n" "$registry_path"
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
  if [[ -f /home/ubuntu/.cargo/env ]]; then
    # shellcheck disable=SC1091
    source /home/ubuntu/.cargo/env
  fi
  if [[ -f /root/.cargo/env ]]; then
    # shellcheck disable=SC1091
    source /root/.cargo/env
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

  seed_builtin_browser_capability
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
