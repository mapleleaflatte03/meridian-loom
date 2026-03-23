# Meridian Loom

Experimental runtime rehearsal for Meridian-native governed execution.

Meridian Loom is the planned execution fabric for Meridian. It is not the live
runtime today. OpenClaw still runs the live host. This repository exists to
make the next runtime concrete now, before any false maturity claims:

- a real `loom` binary
- a real setup path
- real operator surfaces
- real fail-closed rehearsal
- real runtime-side audit artifacts
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
- `loom contract show`
- `loom agent resolve`
- `loom envelope build`
- `loom capsule inspect`
- `loom shadow preflight`
- `loom shadow decide`
- `loom shadow enforce`
- `loom action execute`
- `loom shadow compare`
- `loom shadow report`
- `loom parity report`

There is also a second rehearsal for local sanction denial:

```bash
./scripts/rehearse_local_sanction_preview.sh
```

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

- `Meridian Loom // DOCTOR`
- `Meridian Loom // STATUS`
- `Meridian Loom // CONTRACT`
- `Meridian Loom // AGENT IDENTITY`
- `Meridian Loom // ACTION ENVELOPE`
- `Meridian Loom // CAPSULE INSPECT`
- `Meridian Loom // SHADOW PREFLIGHT`
- `Meridian Loom // SHADOW DECISION`
- `Meridian Loom // RUNTIME EXECUTE`
- `Meridian Loom // SHADOW REPORT`
- `Meridian Loom // PARITY REPORT`

This matters. Loom is not just a crate layout. It is also a future operator
surface, and that surface has to be designed now, not after the runtime exists.

## Current runtime rehearsal status

The important question is not “does Loom have commands?” The important question
is “what parts of a real runtime path are already tangible?”

| Surface | Current truth |
|---|---|
| Native sanction enforcement | `loom action execute` now enforces the current effective allow/deny decision and fails closed with exit code `2` when denied. This is still an experimental runtime rehearsal, not a governed worker supervisor. |
| Runtime-side audit emission | `loom action execute` now writes a runtime audit artifact under `.loom/audit/runtime_events.jsonl`, using the kernel serializer when available and a local fallback otherwise. This is not the kernel's canonical audit log. |
| Parity stream | `loom action execute` now emits `.loom/parity/stream.jsonl` and `.loom/parity/latest.json`. The stream records reference-gate truth, Loom runtime rehearsal truth, audit emission, and an optional live OpenClaw proof snapshot when available. |
| Live OpenClaw reference | On the founder host, Loom now captures a real OpenClaw proof snapshot through `openclaw_runtime_proof.py --json` and stores it under `.loom/parity/openclaw_live.json`. This is live runtime evidence, but not per-action OpenClaw parity yet. |
| Shadow compare | `loom shadow compare` still exists, but it is now explicitly an offline event-log diff, not the main parity story. |

## What does not exist yet

Loom is still missing the things that would make it a real runtime:

- no governed worker supervisor
- no native transport adapters
- no long-running scheduler/runtime loop
- no native sanction enforcement inside a live worker runtime
- no kernel-owned canonical audit trail
- no per-action live OpenClaw parity stream
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
loom action execute
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
- runtime-side audit artifacts
- a parity stream
- honest boundaries about what is still missing

That is enough to start shaping the real runtime without lying about what
already exists.
