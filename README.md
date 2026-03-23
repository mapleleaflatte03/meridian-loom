# Meridian Loom

Experimental public scaffold for the planned Meridian-native runtime.

## Truth first

- This repo is an **experimental scaffold**, not a production runtime.
- Contract compliance remains **0/7 proven hooks**.
- OpenClaw is still the live runtime today.
- Meridian remains the governance kernel above runtimes.
- This repo exists to make the Loom product lane tangible: CLI shape, setup path,
  local state layout, contract inspection, doctor/health surfaces, and setup
  rehearsal.

## What exists in this scaffold

- A Rust workspace with:
  - `loom-core` — config/state helpers, identity resolution, contract inspection, envelope build
  - `loom-cli` — `loom` binary
  - `loom-shadow` — shadow event capture, comparison, and report surfaces
- Working commands:
  - `loom init`
  - `loom doctor`
  - `loom health`
  - `loom status`
  - `loom config show`
  - `loom contract show`
  - `loom agent resolve`
  - `loom envelope build`
  - `loom capsule inspect`
  - `loom shadow preflight`
  - `loom shadow compare`
  - `loom shadow report`
- A local setup rehearsal script:
  - `scripts/rehearse_setup.sh` (auto-discovers a governed agent from the
    current kernel registry before running the experimental preflight flow)

## What does not exist yet

- No governed execution runtime
- No worker supervisor
- No MCP / Telegram / HTTP transport
- No proven runtime hook implementation beyond the experimental preflight path for
  `agent_identity` and `action_envelope`
- No shadow-mode parity engine
- No public benchmark claims

## Quick start

```bash
cargo build
./target/debug/loom init --mode embedded --kernel-path /tmp/meridian-kernel --root /tmp/loom-rehearsal
./target/debug/loom doctor --root /tmp/loom-rehearsal --format human
./target/debug/loom health --root /tmp/loom-rehearsal --format json
./target/debug/loom contract show --root /tmp/loom-rehearsal
./target/debug/loom agent resolve --root /tmp/loom-rehearsal --agent-id agent_atlas --format human
./target/debug/loom envelope build --root /tmp/loom-rehearsal --agent-id agent_atlas --action-type research --resource web_search --estimated-cost-usd 0.05 --format human
./target/debug/loom capsule inspect --root /tmp/loom-rehearsal
./target/debug/loom shadow preflight --root /tmp/loom-rehearsal --agent-id agent_atlas --action-type research --resource web_search --estimated-cost-usd 0.05 --format human
./target/debug/loom shadow compare --root /tmp/loom-rehearsal --primary /tmp/loom-rehearsal/.loom/shadow/events.jsonl --shadow /tmp/loom-rehearsal/.loom/shadow/events.jsonl --format human
./target/debug/loom shadow report --root /tmp/loom-rehearsal
```

If your kernel registry constrains agent lookup by organization, pass the bound
org explicitly with `--org-id <bound-org-id>`. The bundled rehearsal script
auto-discovers both the agent id and org id from the current kernel registry.

Or run the bundled rehearsal:

```bash
./scripts/rehearse_setup.sh
```

Run the publication/readiness check:

```bash
./scripts/check_publication_readiness.sh
```

## Layout

```text
meridian-loom/
  Cargo.toml
  loom.toml.example
  crates/
    loom-cli/
    loom-core/
    loom-shadow/
  docs/
  scripts/
```

## Current status

This repo is enough to rehearse the install/setup/operator path honestly, and
it now includes an experimental preflight path for `agent_identity` and
`action_envelope`.
It is still not enough to claim Loom exists as a governed execution runtime.

## Publication readiness

This scaffold is now public at
`https://github.com/mapleleaflatte03/meridian-loom`. The publication checklist lives in
[`docs/PUBLICATION_CHECKLIST.md`](docs/PUBLICATION_CHECKLIST.md), and the
bundled readiness script verifies the local prerequisites, remote wiring, and
public repository visibility.
