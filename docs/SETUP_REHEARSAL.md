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
7. `loom capsule inspect` surfaces the local capsule state boundary.
8. `loom shadow report` reports the current placeholder shadow state honestly.

## What the rehearsal does not prove

- It does not prove any contract hook is implemented.
- It does not prove transport adapters exist.
- It does not prove OpenClaw replacement.
- It does not prove shadow parity.

## Run

```bash
./scripts/rehearse_setup.sh
```

The script creates a disposable directory under `/tmp/loom-rehearsal` by
default and does not mutate the Meridian kernel.

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
