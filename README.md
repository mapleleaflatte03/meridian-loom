<p align="center">
  <img src="./docs/assets/meridian_loom_lockup.svg" alt="Meridian Loom" width="720">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-1f6feb?style=flat-square" alt="MIT license">
  <img src="https://img.shields.io/github/actions/workflow/status/mapleleaflatte03/meridian-loom/rust.yml?branch=main&style=flat-square" alt="Build passing">
  <img src="https://img.shields.io/badge/version-0.1.16-0c1117?style=flat-square" alt="Version 0.1.16">
</p>

<p align="center">
  <strong>Install in one command. Run a governed local agent in minutes. Inspect the receipt instead of guessing.</strong>
</p>

<p align="center">
  <img src="./docs/assets/install_in_60_seconds.gif" alt="Install Loom in one command, then run doctor and status to see the proof-first runtime surface." width="920">
</p>

<p align="center">
  <a href="docs/INSTALL.md">Install</a> ·
  <a href="docs/QUICKSTART.md">Quickstart</a> ·
  <a href="docs/RUN_LOCAL.md">Run Local</a> ·
  <a href="docs/BENCHMARKS.md">Benchmarks</a> ·
  <a href="docs/RELEASE.md">Release</a> ·
  <a href="docs/SERVICE.md">Service</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="docs/MERIDIAN_PoGE_PROTOCOL.md">PoGE</a>
</p>

# Meridian Loom – Governed Local Agent Runtime v0.1

Loom is the governed local runtime for AI agents. Install in one command, run
terminal/browser/schedule/personal-agent jobs, and inspect proof receipts
immediately. No vibes, just proof + governance.

If you want the shortest honest summary:

- **Install:** one command, binary first
- **Create:** `loom new-agent` provisions a governed personal agent with Kernel wiring
- **Operate:** one CLI for doctor, status, jobs, queue, parity, service, memory, and channels
- **Prove:** every governed execution and memory event emits receipts and proof views
- **Boundary:** the local runtime is real; broader hosted replacement is not claimed here

## 1-command install

```bash
curl -fsSL https://raw.githubusercontent.com/mapleleaflatte03/meridian-loom/main/scripts/install.sh | bash
```

The installer prefers prebuilt GitHub release assets for the current host and
falls back to a source build only when no matching asset is available.

## Create your first governed personal agent

```bash
export LOOM_ROOT="${HOME}/.local/share/meridian-loom/runtime/default"
export MERIDIAN_KERNEL_PATH=/opt/meridian-kernel
export MERIDIAN_ORG_ID="${MERIDIAN_ORG_ID:-local_foundry}"

loom new-agent \
  --name "My Assistant" \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --org-id "$MERIDIAN_ORG_ID"

loom run-agent my-assistant
```

What that does:

- initializes Loom if needed
- registers the agent in Kernel with `runtime_binding=loom_native`
- creates `~/.config/meridian-loom/agents/my-assistant/`
- writes `agent.toml`, `README.md`, `MEMORY.md`, and `SOUL.md`
- starts a persistent heartbeat-driven governed loop when you run the agent

Inspect it:

```bash
loom status --root "$LOOM_ROOT"
loom channel deliveries --root "$LOOM_ROOT" --include-archived
```

The full end-to-end flow lives in [docs/QUICKSTART.md](docs/QUICKSTART.md).

## What ships in the official v0.1 release

- One-command installer with binary-first release installs
- Prebuilt GitHub release assets for:
  - Linux x86_64
  - Linux arm64
  - macOS x86_64
  - macOS arm64
- Local runtime root under `$HOME/.local/share/meridian-loom`
- `loom` linked into `$HOME/.local/bin`
- Built-in governed capabilities for:
  - terminal execution
  - browser navigation
  - heartbeat scheduling
- Personal agent commands:
  - `loom new-agent`
  - `loom run-agent`
- Queue, job, audit, parity, and runtime-service surfaces on disk
- Memory receipts for write/read/remove/prune operations
- Proof of Governed Execution (PoGE) contract and receipt architecture

## Quickstart: three copy-paste examples

These examples are intentionally concrete. They assume:

- you installed `loom`
- your Kernel repo is available at `/opt/meridian-kernel`
- you are willing to keep the truth boundary explicit
- you are fine using `local_foundry` as the default local org id on a fresh root

If you have not initialized the Kernel yet, run the personal-agent flow above or example 3 first.

### 1. Run one terminal job and inspect the receipt

```bash
export LOOM_ROOT="${HOME}/.local/share/meridian-loom/runtime/default"
export MERIDIAN_KERNEL_PATH=/opt/meridian-kernel
export MERIDIAN_ORG_ID="${MERIDIAN_ORG_ID:-local_foundry}"

loom init \
  --root "$LOOM_ROOT" \
  --mode embedded \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --org-id "$MERIDIAN_ORG_ID"

loom doctor --root "$LOOM_ROOT" --format human

loom action execute \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --agent-id agent_atlas \
  --org-id "$MERIDIAN_ORG_ID" \
  --capability loom.terminal.exec.v1 \
  --payload-json '{"argv":["bash","-lc","printf \"hello from loom\\n\""],"working_dir":".","timeout_ms":2000,"max_output_bytes":4096}' \
  --estimated-cost-usd 0.05

LATEST_JOB_ID="$(
  loom job list --root "$LOOM_ROOT" --format json \
    | python3 -c 'import json,sys; jobs=json.load(sys.stdin).get("jobs", []); print(jobs[0]["job_id"] if jobs else "")'
)"

loom job inspect --root "$LOOM_ROOT" --job-id "$LATEST_JOB_ID" --format human
loom parity report --root "$LOOM_ROOT"
loom shadow report --root "$LOOM_ROOT"
```

What you should see:

- a governed decision
- a runtime execution receipt
- audit/parity artifact paths
- one obvious next step if the run degraded

### 2. Run bounded browser navigation and inspect proof

```bash
export LOOM_ROOT="${HOME}/.local/share/meridian-loom/runtime/default"
export MERIDIAN_KERNEL_PATH=/opt/meridian-kernel
export MERIDIAN_ORG_ID="${MERIDIAN_ORG_ID:-local_foundry}"

loom action execute \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --agent-id agent_atlas \
  --org-id "$MERIDIAN_ORG_ID" \
  --capability loom.browser.navigate.v1 \
  --payload-json '{"session_id":"docs-example","url":"https://example.com","allowed_hosts":["example.com"],"wait_for":"dom_content_loaded","timeout_ms":4000,"capture_semantic_snapshot":true}' \
  --estimated-cost-usd 0.05

LATEST_JOB_ID="$(
  loom job list --root "$LOOM_ROOT" --format json \
    | python3 -c 'import json,sys; jobs=json.load(sys.stdin).get("jobs", []); print(jobs[0]["job_id"] if jobs else "")'
)"

loom job inspect --root "$LOOM_ROOT" --job-id "$LATEST_JOB_ID" --format human
loom parity report --root "$LOOM_ROOT"
```

This proves the local browser host-call lane and its receipt surfaces. It does
not claim broad hosted browser automation.

### 3. Connect Loom to Kernel using `quickstart.py`

```bash
cd /opt/meridian-kernel
python3 quickstart.py --init-only

export LOOM_ROOT="${HOME}/.local/share/meridian-loom/runtime/default"
export MERIDIAN_ORG_ID="${MERIDIAN_ORG_ID:-local_foundry}"
loom init \
  --root "$LOOM_ROOT" \
  --mode embedded \
  --kernel-path /opt/meridian-kernel \
  --org-id "$MERIDIAN_ORG_ID"

loom contract show --root "$LOOM_ROOT" --kernel-path /opt/meridian-kernel
loom doctor --root "$LOOM_ROOT" --format human
```

If you want the Kernel demo dashboard as well:

```bash
cd /opt/meridian-kernel
python3 quickstart.py --port 8080
```

That gives you the governed workspace while Loom remains the local execution
surface.

## Doctor and status should tell you what to do next

The first-run commands worth memorizing are:

```bash
loom doctor --root "$HOME/.local/share/meridian-loom/runtime/default" --format human
loom status --root "$HOME/.local/share/meridian-loom/runtime/default"
```

The goal of both commands is simple:

- `doctor` tells you whether the runtime is ready, degraded, or blocked
- `status` tells you where the runtime, queue, service, and agent artifacts live
- both should point to an obvious next command instead of forcing you to read the source

## Benchmark harness

Loom now ships a tiny reproducible benchmark harness at
[`scripts/bench_runtime.py`](scripts/bench_runtime.py). It measures short-lived
CLI cold starts and approximate peak RSS on the same host.

Example:

```bash
python3 scripts/bench_runtime.py \
  --iterations 5 \
  --warmup 1 \
  --case "loom status::./target/release/loom status --root /tmp/loom-bench-root" \
  --case "openfang help::openfang --help" \
  --case "ironclaw::ironclaw --help" \
  --format markdown
```

Current reference run on the Meridian VPS (`2026-04-01`):

| Case | Mean cold start (ms) | p95 (ms) | Peak RSS (MiB) |
| --- | ---: | ---: | ---: |
| `loom status` | 29.8 | 33.9 | 4.8 |
| `openfang --help` | 28.5 | 30.8 | 2.8 |
| `ironclaw --help` | 32.6 | 33.9 | 5.1 |

Why this exists:

- OpenFang is strong on one-binary operator packaging
- IronClaw is strong on secure local-assistant ergonomics
- Loom should make the comparison reproducible on one machine, with one script,
  and with a clear boundary around what the numbers do and do not mean

See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for the benchmark boundary and the
recommended command choices.

## Release story

Loom releases are GitHub-first operator packages. Tagging `v0.1.16` builds and
publishes release archives for:

- Linux x86_64
- Linux arm64
- macOS x86_64
- macOS arm64

Each asset includes the Loom binary, example config, docs, installer helpers,
systemd units, and a checksum file.

See [docs/RELEASE.md](docs/RELEASE.md) for the release layout.

## Meridian stack

- [meridian-loom](https://github.com/mapleleaflatte03/meridian-loom): official first-party governed local runtime
- [meridian-kernel](https://github.com/mapleleaflatte03/meridian-kernel): runtime-neutral governance, policy, authority, treasury, and court
- [meridian-intelligence](https://github.com/mapleleaflatte03/meridian-intelligence): first-party workflows and public product surfaces built on Loom + Kernel

## Proof of Governed Execution (PoGE)

The cryptographic execution-receipt architecture is defined in the
[Meridian PoGE Protocol RFC](docs/MERIDIAN_PoGE_PROTOCOL.md).

- Purpose: bind governed host-calls to verifiable receipts, Merkle roots, and future settlement surfaces
- Scope: Loom runtime host-call evidence and audit architecture
- Boundary: this repo does not claim that every future settlement primitive is already live here

## Truth boundary

- Local install, queue, audit, parity, and runtime-service surfaces are real
- Terminal execution, browser navigation, and heartbeat scheduling are real local primitives
- Hosted runtime replacement is not claimed here
- Multi-channel presence and memory should only be claimed through named proof surfaces, not vague runtime language
- Compatibility and migration surfaces exist where they still help real operator cutovers; they do not turn Loom into a broad hosted-runtime claim

## Operational surface

- `loom init`, `loom doctor`, `loom health`, `loom status`
- `loom start`, `loom stop`, `loom restart`, `loom logs`
- `loom queue inspect|consume|ack|run-once|run-until-empty|status`
- `loom job list|inspect`
- `loom parity report`
- `loom shadow report`
- `loom capability list|show|gap show|scaffold|forge|import-workspace-skill|verify|promote|shim`
- `loom service start|status|submit|import-commitments|stop`

## Rehearsals

- Operational rehearsals live in `scripts/tests/`
- Migration and backward-compatibility rehearsals live in `scripts/migration_tools/`
- Generated `examples/*-output.txt` transcripts are not checked in
