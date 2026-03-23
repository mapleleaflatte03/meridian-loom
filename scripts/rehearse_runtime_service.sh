#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${1:-/tmp/loom-runtime-service}"
KERNEL_PATH="$(mktemp -d /tmp/loom-runtime-service-kernel.XXXXXX)"
SOURCE_KERNEL="${SOURCE_KERNEL:-/tmp/meridian-kernel}"

cleanup() {
  rm -rf "${ROOT_DIR}" "${KERNEL_PATH}"
}
trap cleanup EXIT

echo "== Meridian Loom runtime service rehearsal =="
echo "root:   ${ROOT_DIR}"
echo "kernel: ${KERNEL_PATH}"
echo "agent:  agent_allow"
echo "org:    org_demo"

rm -rf "${ROOT_DIR}"
mkdir -p "${KERNEL_PATH}/kernel/adapters"

cat > "${KERNEL_PATH}/kernel/runtimes.json" <<'EOF'
{
  "runtimes": {
    "local_kernel": {"id": "local_kernel", "label": "Local Kernel Runtime"},
    "meridian_loom": {
      "status": "experimental",
      "notes": "fixture-backed runtime service rehearsal",
      "contract_compliance": {
        "agent_identity": null,
        "action_envelope": null,
        "cost_attribution": null,
        "approval_hook": null,
        "audit_emission": null,
        "sanction_controls": null,
        "budget_gate": null
      }
    }
  }
}
EOF

cat > "${KERNEL_PATH}/kernel/agent_registry.py" <<'EOF'
import json
import sys

agent_id = sys.argv[sys.argv.index('--agent_id') + 1]
org_id = 'org_demo'
if '--org_id' in sys.argv:
    org_id = sys.argv[sys.argv.index('--org_id') + 1]

if agent_id in ('agent_allow', 'allow', 'Allow'):
    print(json.dumps({
        'id': 'agent_allow',
        'name': 'Allow Path',
        'org_id': org_id,
        'role': 'analyst',
        'economy_key': 'allow',
        'approval_required': False,
        'restrictions': [],
        'sanction_decision': 'clear',
        'budget': {'max_per_run_usd': 5.0},
        'runtime_binding': {
            'runtime_id': 'local_kernel',
            'runtime_label': 'Local Kernel Runtime',
            'bound_org_id': org_id,
            'boundary_name': 'workspace',
            'identity_model': 'session',
            'runtime_registered': True,
            'registration_status': 'registered'
        }
    }, indent=2))
else:
    print(f'Not found: {agent_id}')
EOF

cat > "${KERNEL_PATH}/kernel/court.py" <<'EOF'
def get_restrictions(agent_id, org_id=None):
    return []
EOF

cat > "${KERNEL_PATH}/kernel/authority.py" <<'EOF'
def check_authority(agent_id, action, org_id=None):
    return True, 'ok'
EOF

cat > "${KERNEL_PATH}/kernel/treasury.py" <<'EOF'
def check_budget(agent_id, cost_usd, org_id=None):
    return True, 'ok'

def reserve_runtime_budget(agent_id, estimated_cost_usd, org_id=None, action='', resource='', context=None, policy_ref='', lease_seconds=60):
    return {
        'allowed': True,
        'reservation_id': 'bud_service',
        'reservation': {
            'reservation_id': 'bud_service',
            'status': 'reserved',
            'estimated_cost_usd': estimated_cost_usd
        },
        'reason': 'ok',
    }

def commit_runtime_budget(reservation_id, actual_cost_usd, note=''):
    return {
        'reservation_id': reservation_id,
        'status': 'committed',
        'commit_reason': note,
    }

def release_runtime_budget(reservation_id, reason=''):
    return {
        'reservation_id': reservation_id,
        'status': 'released',
        'release_reason': reason,
    }
EOF

cp "${SOURCE_KERNEL}/kernel/audit.py" "${KERNEL_PATH}/kernel/audit.py"

cat > "${KERNEL_PATH}/kernel/metering.py" <<'EOF'
def record(*args, **kwargs):
    return 'meter_fixture'
EOF

cat > "${KERNEL_PATH}/kernel/adapters/__init__.py" <<'EOF'
# fixture package
EOF

cat > "${KERNEL_PATH}/kernel/adapters/openclaw_compatible.py" <<'EOF'
from authority import check_authority
from court import get_restrictions
from treasury import check_budget

def pre_session_check(org_id, agent_id):
    restrictions = list(get_restrictions(agent_id, org_id=org_id) or [])
    if 'execute' in restrictions or 'remediation_only' in restrictions:
        return {'allowed': False, 'reason': f'Agent {agent_id} is restricted from execute', 'restrictions': restrictions}
    return {'allowed': True, 'reason': 'ok', 'restrictions': restrictions}

def pre_action_check(org_id, envelope):
    session_gate = pre_session_check(org_id, envelope['agent_id'])
    if not session_gate['allowed']:
        return {'allowed': False, 'reason': session_gate['reason'], 'stage': 'sanction_controls', 'envelope': envelope, 'restrictions': session_gate['restrictions']}
    allowed, reason = check_authority(envelope['agent_id'], envelope['action_type'], org_id=org_id)
    if not allowed:
        return {'allowed': False, 'reason': reason, 'stage': 'approval_hook', 'envelope': envelope, 'restrictions': session_gate['restrictions']}
    estimated_cost = envelope.get('estimated_cost_usd', 0.0)
    if estimated_cost > 0:
        allowed, reason = check_budget(envelope['agent_id'], estimated_cost, org_id=org_id)
        if not allowed:
            return {'allowed': False, 'reason': reason, 'stage': 'budget_gate', 'envelope': envelope, 'restrictions': session_gate['restrictions']}
    return {'allowed': True, 'reason': 'ok', 'stage': 'ok', 'envelope': envelope, 'restrictions': session_gate['restrictions']}
EOF

cargo build --workspace
export MERIDIAN_OPENCLAW_PROOF_SCRIPT="${KERNEL_PATH}/kernel/missing_openclaw_runtime_proof.py"

./target/debug/loom init --mode embedded --kernel-path "${KERNEL_PATH}" --root "${ROOT_DIR}" --org-id org_demo
./target/debug/loom service start --root "${ROOT_DIR}" --kernel-path "${KERNEL_PATH}" --max-jobs 1 --poll-seconds 1 --iterations 20 --format human
for _ in $(seq 1 20); do
  if [[ -f "${ROOT_DIR}/.loom/runtime/service/runtime_state.json" ]]; then
    break
  fi
  sleep 0.2
done

./target/debug/loom service submit --root "${ROOT_DIR}" --agent-id agent_allow --org-id org_demo --action-type research --resource web_search --estimated-cost-usd 0.05 --format human
sleep 2
./target/debug/loom service status --root "${ROOT_DIR}" --format human
./target/debug/loom job list --root "${ROOT_DIR}" --format human
./target/debug/loom parity report --root "${ROOT_DIR}"
./target/debug/loom service stop --root "${ROOT_DIR}" --format human
sleep 1
./target/debug/loom service status --root "${ROOT_DIR}" --format human

echo "service_runtime_state:"
cat "${ROOT_DIR}/.loom/runtime/service/runtime_state.json"
echo "service_events:"
cat "${ROOT_DIR}/.loom/runtime/service/service_events.jsonl"
echo "ingress_stream:"
cat "${ROOT_DIR}/.loom/runtime/ingress/stream.jsonl"

echo "== Runtime service rehearsal complete =="
