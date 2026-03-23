# Setup Rehearsal

This repository includes a local setup rehearsal so the Loom install path can be
tested before any public release claims are made.

## Current rehearsal scope

The rehearsal verifies:

1. The Rust workspace builds.
2. The Rust workspace tests pass.
3. `loom init` creates config and local state.
4. `loom doctor` reports configuration and filesystem health.
5. `loom health` returns a structured summary.
6. `loom contract show` can read the current kernel runtime registry.
7. `loom agent resolve` resolves a governed agent identity against the kernel registry.
8. `loom envelope build` constructs a normalized action envelope.
9. `loom capsule inspect` surfaces the local capsule state boundary.
10. `loom shadow preflight` captures experimental shadow events for all seven
    contract surfaces.
11. `audit_emission` is currently exercised as a local audit preview file, not
    the kernel's canonical audit log.
12. `sanction_controls`, `approval_hook`, and `budget_gate` are now evaluated
    through the kernel reference adapter in a read-only preflight path, but not
    through a native Loom runtime.
13. `loom shadow compare` now compares reference-adapter event decisions
    against Loom's captured shadow events.
14. `loom shadow report` surfaces the latest shadow capture or comparison report honestly.

## What the rehearsal does not prove

- It does not prove runtime-level contract compliance.
- It does not upgrade registry compliance beyond 0/7.
- It does not prove transport adapters exist.
- It does not prove OpenClaw replacement.
- It does not prove runtime parity.
- The comparison surface is still file-level and adapter-backed, not live runtime parity.

## Run

```bash
./scripts/rehearse_setup.sh
```

The script creates a disposable directory under `/tmp/loom-rehearsal` by
default, auto-discovers a governed agent from the current kernel registry, and
does not mutate the Meridian kernel.

## Fresh public clone verification

After the first public push, the scaffold was re-verified from a clean clone of
`https://github.com/mapleleaflatte03/meridian-loom.git` on the founder host.

The verification path was:

```bash
git clone https://github.com/mapleleaflatte03/meridian-loom.git /tmp/meridian-loom-clone/repo
cd /tmp/meridian-loom-clone/repo
cargo test
cargo build
./scripts/rehearse_setup.sh
```

That fresh-clone run passed and confirmed:

1. The public repository builds from scratch.
2. The public repository tests pass from scratch.
3. The bundled rehearsal still succeeds against the current kernel truth.
4. The scaffold still reports `planned` runtime status and `0/7` proven hooks.

The current rehearsal now also exercises `loom shadow compare` against a
reference event log generated from the kernel-side OpenClaw-compatible adapter.
That makes the divergence surface more useful without turning it into a live
runtime parity claim.
