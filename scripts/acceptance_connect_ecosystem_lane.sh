#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/loom_connect_ecosystem_lane.XXXXXX)"
trap 'rm -rf "${TMP_DIR}"' EXIT

HOME_DIR="${TMP_DIR}/home"
ROOT_DIR="${HOME_DIR}/.local/share/meridian-loom/runtime/default"
KERNEL_DIR="${TMP_DIR}/kernel"
mkdir -p "${HOME_DIR}" "${KERNEL_DIR}/kernel"

cat > "${KERNEL_DIR}/kernel/runtimes.json" <<'JSON'
{
  "runtimes": {
    "local_kernel": { "id": "local_kernel", "label": "Local Kernel Runtime" },
    "loom_native": {
      "status": "official",
      "notes": "connect acceptance lane",
      "contract_compliance": {
        "agent_identity": true,
        "action_envelope": true,
        "cost_attribution": true,
        "approval_hook": true,
        "audit_emission": true,
        "sanction_controls": true,
        "budget_gate": true
      }
    }
  }
}
JSON

cat > "${KERNEL_DIR}/kernel/agent_registry.py" <<'PY'
import json
import sys
agent_id = sys.argv[sys.argv.index("--agent_id") + 1]
org_id = sys.argv[sys.argv.index("--org_id") + 1] if "--org_id" in sys.argv else "org_demo"
print(json.dumps({
    "id": agent_id,
    "name": "Atlas",
    "org_id": org_id,
    "role": "analyst",
    "economy_key": "atlas",
    "approval_required": False,
    "budget": {"max_per_run_usd": 1.0},
    "runtime_binding": {
        "runtime_id": "loom_native",
        "runtime_label": "Loom Native Runtime",
        "bound_org_id": org_id,
        "boundary_name": "workspace",
        "identity_model": "session",
        "runtime_registered": True,
        "registration_status": "registered"
    }
}, indent=2))
PY

cat > "${KERNEL_DIR}/kernel/court.py" <<'PY'
def get_restrictions(agent_id, org_id=None):
    return []
PY

cat > "${KERNEL_DIR}/kernel/authority.py" <<'PY'
def check_authority(agent_id, action, org_id=None):
    return True, "ok"
PY

echo "[loom-acceptance] build loom binary"
(cd "${REPO_ROOT}" && cargo build -p meridian-loom --quiet)
LOOM_BIN="${REPO_ROOT}/target/debug/loom"

export HOME="${HOME_DIR}"
export XDG_CONFIG_HOME="${HOME_DIR}/.config"
mkdir -p "${XDG_CONFIG_HOME}"

run_loom() {
  "${LOOM_BIN}" "$@"
}

run_loom_json() {
  run_loom "$@" --format json
}

echo "[loom-acceptance] init runtime root"
run_loom init --mode embedded --root "${ROOT_DIR}" --kernel-path "${KERNEL_DIR}" --org-id org_demo >/dev/null

declare -a ADAPTERS=(
  "grpc_adapter grpc"
  "a2a_adapter a2a"
  "mcp_adapter mcp"
  "http_adapter http"
  "ros2_adapter ros2"
)

echo "[loom-acceptance] scaffold + validate + enable + test + health for all transports"
for pair in "${ADAPTERS[@]}"; do
  name="${pair%% *}"
  transport="${pair##* }"
  adapter_id="${name//_/-}"

  run_loom_json connect scaffold --name "${name}" --transport "${transport}" --action-schema meridian.runtime.v1 --root "${ROOT_DIR}" >/dev/null
  run_loom_json connect validate --adapter-id "${adapter_id}" --root "${ROOT_DIR}" >/dev/null
  run_loom_json connect enable --adapter-id "${adapter_id}" --root "${ROOT_DIR}" >/dev/null
  run_loom_json connect test --adapter-id "${adapter_id}" --root "${ROOT_DIR}" >/dev/null
  run_loom_json connect health --adapter-id "${adapter_id}" --root "${ROOT_DIR}" >/dev/null
done

echo "[loom-acceptance] assert disabled adapter test fail path"
run_loom_json connect disable --adapter-id grpc-adapter --root "${ROOT_DIR}" >/dev/null
set +e
FAIL_OUTPUT="$(run_loom_json connect test --adapter-id grpc-adapter --root "${ROOT_DIR}" 2>&1)"
FAIL_STATUS=$?
set -e
if [[ ${FAIL_STATUS} -eq 0 ]]; then
  echo "expected connect test fail path for disabled adapter, but command succeeded" >&2
  exit 1
fi
if [[ "${FAIL_OUTPUT}" != *"adapter_disabled"* ]]; then
  echo "expected adapter_disabled fail reason, got:" >&2
  echo "${FAIL_OUTPUT}" >&2
  exit 1
fi

echo "[loom-acceptance] verify diagnostics + artifacts persisted"
python3 - <<'PY' "${ROOT_DIR}"
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
adapters = ["grpc-adapter", "a2a-adapter", "mcp-adapter", "http-adapter", "ros2-adapter"]
for adapter in adapters:
    health_path = root / "state/connect/health" / f"{adapter}.json"
    tests_path = root / "state/connect/tests" / f"{adapter}.jsonl"
    if not health_path.exists():
        raise SystemExit(f"missing health artifact: {health_path}")
    if not tests_path.exists():
        raise SystemExit(f"missing tests history: {tests_path}")
    if not tests_path.read_text().strip():
        raise SystemExit(f"empty tests history: {tests_path}")

latest_path = root / "artifacts/connect/latest.json"
if not latest_path.exists():
    raise SystemExit(f"missing latest artifact: {latest_path}")
latest = json.loads(latest_path.read_text())
if latest.get("status") not in {"connect_tested", "connect_health", "connect_disabled", "connect_enabled", "connect_validated", "connect_scaffolded"}:
    raise SystemExit(f"unexpected latest status: {latest.get('status')}")
print("artifacts_ok")
PY

echo "[loom-acceptance] PASS connect ecosystem lane"
