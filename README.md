<p align="center">
  <img src="docs/assets/meridian_loom_lockup.svg" alt="Meridian Loom" width="720">
</p>

<p align="center">
  Experimental runtime rehearsal for Meridian-native governed execution.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/phase-0%20%2F%20runtime%20rehearsal-0c1117?style=flat-square" alt="Phase 0 runtime rehearsal">
  <img src="https://img.shields.io/badge/contract-0%2F7%20proven-8b0000?style=flat-square" alt="0/7 proven hooks">
  <img src="https://img.shields.io/github/actions/workflow/status/mapleleaflatte03/meridian-loom/rust.yml?branch=main&style=flat-square" alt="Rust CI">
  <img src="https://img.shields.io/badge/license-MIT-1f6feb?style=flat-square" alt="MIT license">
  <img src="https://img.shields.io/badge/repo-public-0f766e?style=flat-square" alt="Public repository">
</p>

<p align="center">
  <a href="docs/SETUP_REHEARSAL.md">Setup Rehearsal</a> ·
  <a href="docs/PUBLICATION_CHECKLIST.md">Publication Checklist</a> ·
  <a href="https://github.com/mapleleaflatte03/meridian-kernel">Meridian Kernel</a> ·
  <a href="https://github.com/mapleleaflatte03/meridian-kernel/blob/main/docs/LOOM_SPEC.md">Loom Spec</a> ·
  <a href="https://app.welliam.codes">Live Host</a>
</p>

<p align="center">
  <img src="docs/assets/loom_runtime_panels.svg" alt="Meridian Loom runtime rehearsal surfaces" width="960">
</p>

> Phase 0 today is not “a runtime is done.” It is “the runtime surface has become inspectable”: buildable binary, real setup path, real operator grammar, fail-closed rehearsal, runtime-side audit artifacts, and parity surfaces with honest limits.

# Meridian Loom

Meridian Loom is the planned execution fabric for Meridian. It is not the live
runtime today. OpenClaw still runs the live host. This repository exists to
make the next runtime concrete now, before any false maturity claims:

- a real `loom` binary
- a real setup path
- real operator surfaces
- real fail-closed rehearsal
- real kernel-owned runtime audit artifacts
- a real parity stream
- honest proof boundaries

## Truth boundary

- This repo is a **public experimental scaffold**, not a production runtime.
- Registry compliance remains **0/7 proven hooks**.
- Loom does **not** replace OpenClaw today.
- Meridian remains the governance kernel above runtimes.
- The current value of this repo is product shape, operator shape, and runtime
  rehearsal, not benchmark theater.

## Why Loom exists

Meridian Kernel governs digital labor. It does not execute it.

That split is deliberate:

- **Kernel** owns institution, authority, treasury, court, and runtime contract
- **Loom** will own lifecycle, execution, worker orchestration, transport, and
  native enforcement

OpenClaw is the current live runtime. Loom exists because Meridian should not
stay adapter-defined forever. It needs its own execution fabric, its own
operator language, and its own fail-closed runtime path.

## Quick start

If you want to evaluate Loom today, do this first:

```bash
git clone https://github.com/mapleleaflatte03/meridian-loom.git
cd meridian-loom
cargo build
./scripts/rehearse_setup.sh
```

That one rehearsal gives you:

- `loom init`
- `loom doctor`
- `loom health`
- `loom status`
- `loom config show`
- `loom contract show`
- `loom agent resolve`
- `loom envelope build`
- `loom capsule inspect`
- `loom action enqueue`
- `loom shadow preflight`
- `loom shadow decide`
- `loom shadow enforce`
- `loom action execute`
- `loom supervisor run`
- `loom supervisor watch`
- `loom supervisor status`
- `loom shadow compare`
- `loom shadow report`
- `loom parity report`

There is also a second rehearsal for local sanction denial:

```bash
./scripts/rehearse_local_sanction_preview.sh
```

And a third rehearsal for the allow path:

```bash
./scripts/rehearse_allow_execute.sh
```

And a fourth rehearsal for the queue-backed supervisor path:

```bash
./scripts/rehearse_supervisor_queue.sh
```

And a fifth rehearsal for the bounded supervisor watch loop:

```bash
./scripts/rehearse_supervisor_watch.sh
```

Its checked-in transcript lives at `examples/supervisor-watch-output.txt`.

## What exists today

### Product surfaces

- `loom-core`
  - config and local state
  - governed identity resolution
  - contract inspection
  - action envelope construction
  - capsule inspection
- `loom-shadow`
  - shadow preflight capture
  - decision capture
  - fail-closed shell gate
  - runtime execution rehearsal receipt
  - parity stream
  - report surfaces
- `loom-cli`
  - the public `loom` command that drives all of the above

### Operator surfaces

Current human-mode output uses a single grammar:

- `Meridian Loom // INIT`
- `Meridian Loom // DOCTOR`
- `Meridian Loom // HEALTH`
- `Meridian Loom // STATUS`
- `Meridian Loom // CONFIG`
- `Meridian Loom // CONTRACT`
- `Meridian Loom // AGENT IDENTITY`
- `Meridian Loom // ACTION ENVELOPE`
- `Meridian Loom // CAPSULE INSPECT`
- `Meridian Loom // SHADOW PREFLIGHT`
- `Meridian Loom // SHADOW DECISION`
- `Meridian Loom // RUNTIME EXECUTE`
- `Meridian Loom // SHADOW REPORT`
- `Meridian Loom // PARITY REPORT`
- `Meridian Loom // HELP`

This matters. Loom is not just a crate layout. It is also a future operator
surface, and that surface has to be designed now, not after the runtime exists.
When run on a real TTY, the CLI now adds a restrained ANSI shell layer for
headers and status cues. `NO_COLOR=1` disables that layer without changing the
underlying artifact grammar, and `FORCE_COLOR=1` forces it for capture/demo
work.

## Current runtime rehearsal status

The important question is not “does Loom have commands?” The important question
is “what parts of a real runtime path are already tangible?”

| Surface | Current truth |
|---|---|
| Governed local supervisor | `loom action execute` now dispatches an experimental local Python worker when the effective decision is `allow`, and still fails closed with exit code `2` when denied. This is a real local supervisor path, not a hosted runtime replacement. |
| Queue-backed supervisor lane | `loom action enqueue` and `loom supervisor run` now provide a real local queue boundary. A queued action is materialized under `.loom/runtime/queue/`, then processed through the same decision surface and worker dispatch path when the supervisor runs. |
| Supervisor watch loop | `loom supervisor watch` now runs that same queue supervisor in a bounded polling loop, writes `.loom/runtime/supervisor/status.json`, and appends heartbeat history into `.loom/runtime/supervisor/heartbeat.jsonl`. This makes local supervisor state inspectable, but it is still not a daemonized or hosted scheduler. |
| Runtime-side audit emission | `loom action execute` and `loom supervisor run` now write runtime audit entries through the kernel-owned `audit.py log-runtime` path into `kernel/runtime_audit/loom_runtime_events.jsonl` when a kernel is present. This is a canonical kernel-owned file for the current rehearsal boundary, but still not the hosted kernel's global audit trail. |
| Parity stream | `loom action execute` now emits `.loom/parity/stream.jsonl` and `.loom/parity/latest.json`. The stream records reference-gate truth, Loom runtime execution truth, worker status, audit emission, and live-probe status. |
| Live OpenClaw reference | On the founder host, Loom now captures a per-action OpenClaw proof artifact under `.loom/parity/openclaw/<input_hash>.json` plus `.loom/parity/openclaw_live_stream.jsonl`. This is live runtime evidence, but still not hosted per-action parity against an OpenClaw execution action. |
| Shadow compare | `loom shadow compare` still exists, but it is now explicitly an offline event-log diff, not the main parity story. |

## What does not exist yet

Loom is still missing the things that would make it a real runtime:

- no unbounded or daemonized governed worker supervisor
- no hosted supervisor loop or scheduler
- no native transport adapters
- no long-running scheduler/runtime loop
- no native sanction enforcement inside a hosted worker runtime
- no hosted kernel-owned canonical audit trail
- no per-action live OpenClaw parity against a real OpenClaw action execution stream
- no proven 7/7 contract compliance

That is why the registry stays at `0/7`. The scaffold has become more real, but
the proof boundary is still strict.

## Command surface

### Current scaffold commands

```text
loom init
loom doctor
loom health
loom status
loom config show
loom contract show
loom agent resolve
loom envelope build
loom capsule inspect
loom action enqueue
loom action execute
loom supervisor run
loom supervisor watch
loom supervisor status
loom shadow preflight
loom shadow decide
loom shadow enforce
loom shadow compare
loom shadow report
loom parity report
```

### The most important current command

If you only run one thing after setup, run this:

```bash
./target/debug/loom action execute \
  --root /tmp/loom-rehearsal \
  --agent-id agent_atlas \
  --org-id org_b7d95bae \
  --action-type research \
  --resource web_search \
  --estimated-cost-usd 0.05 \
  --format human
```

That one command does four useful things:

1. resolves the governed identity
2. evaluates the current effective decision surface
3. writes a runtime execution receipt and audit artifact
4. updates the parity stream and parity report

If the effective decision is deny, the command exits `2` fail-closed.

If you run `loom parity report` before any runtime rehearsal artifacts exist,
the command now tells you exactly what to run next instead of failing with a
missing-file error.

## What “user only needs Loom” means right now

Today it means:

- you can clone this repo
- you can build the binary
- you can initialize a local boundary
- you can inspect the kernel contract
- you can rehearse the operator path
- you can observe how Loom would fail closed under governance pressure

It does **not** mean:

- you get a replacement for OpenClaw
- you get a supervised worker runtime
- you get native transport ingress
- you get independent institutional deployment

This is still Phase 0: product lane, operator lane, and runtime rehearsal.

## Repository layout

```text
meridian-loom/
  Cargo.toml
  loom.toml.example
  crates/
    loom-cli/
    loom-core/
    loom-shadow/
  docs/
  examples/
  scripts/
```

## Read this next

- [docs/SETUP_REHEARSAL.md](docs/SETUP_REHEARSAL.md)
- [docs/PUBLICATION_CHECKLIST.md](docs/PUBLICATION_CHECKLIST.md)
- [`meridian-kernel` README](https://github.com/mapleleaflatte03/meridian-kernel)
- [`docs/LOOM_SPEC.md` in meridian-kernel](https://github.com/mapleleaflatte03/meridian-kernel/blob/main/docs/LOOM_SPEC.md)

## Bottom line

Meridian Loom is no longer “just a name in a spec.” It is now a public runtime
rehearsal surface with:

- a buildable binary
- a real setup path
- a real operator grammar
- a fail-closed decision path
- a bounded local supervisor watch loop with heartbeat and status artifacts
- runtime-side audit artifacts
- a parity stream
- honest boundaries about what is still missing

That is enough to start shaping the real runtime without lying about what
already exists.
