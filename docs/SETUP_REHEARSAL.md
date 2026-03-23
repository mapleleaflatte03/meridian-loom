<p align="center">
  <img src="assets/meridian_loom_lockup.svg" alt="Meridian Loom" width="720">
</p>

<p align="center">
  Rehearsal transcripts for the public scaffold: install path, operator path, fail-closed path, and current parity path.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rehearsal-founder%20host%20plus%20fixture-0c1117?style=flat-square" alt="Founder host plus fixture">
  <img src="https://img.shields.io/badge/parity-live%20snapshot%20not%20replacement-8b0000?style=flat-square" alt="Parity not replacement">
  <img src="https://img.shields.io/badge/operator-path-real-0f766e?style=flat-square" alt="Operator path real">
</p>

<p align="center">
  <a href="../README.md">Loom README</a> ·
  <a href="PUBLICATION_CHECKLIST.md">Publication Checklist</a> ·
  <a href="https://github.com/mapleleaflatte03/meridian-kernel/blob/main/docs/LOOM_SPEC.md">Loom Spec</a> ·
  <a href="https://app.welliam.codes">Live Host</a>
</p>

# Meridian Loom // Setup Rehearsal

This repository includes two rehearsals:

- a founder-host rehearsal against the current kernel truth
- a fixture-backed rehearsal for local sanction denial

The point is not to pretend Loom is already a runtime. The point is to make the
install path, operator path, and fail-closed runtime rehearsal concrete enough
to inspect honestly.

<p align="center">
  <img src="assets/loom_runtime_panels.svg" alt="Meridian Loom rehearsal surfaces" width="960">
</p>

## Current rehearsal scope

The rehearsal verifies:

1. The Rust workspace builds.
2. The Rust workspace tests pass.
3. `loom init` creates config and local state.
4. `loom doctor` reports configuration and filesystem health.
5. `loom health` returns a structured summary in the canonical operator grammar.
6. `loom config show` renders the resolved local boundary and worker paths.
7. `loom contract show` can read the current kernel runtime registry.
8. `loom agent resolve` resolves a governed agent identity against the kernel registry.
9. `loom envelope build` constructs a normalized action envelope.
10. `loom capsule inspect` surfaces the local capsule state boundary.
11. `loom shadow preflight` captures experimental shadow events for all seven
    contract surfaces.
12. `loom shadow decide` writes a standalone decision artifact for the current
    effective allow/deny result.
13. `loom shadow enforce` reuses that same decision surface and exits fail-closed
    (`0` allow, `2` deny).
14. `loom action execute` now materializes a runtime execution receipt instead
    of stopping at a shell preflight gate.
15. `audit_emission` now writes a runtime-side audit artifact at
    `.loom/audit/runtime_events.jsonl`, using the kernel serializer when
    available and a local fallback otherwise.
16. `loom shadow compare` still exists for offline diffing of event logs.
17. `loom parity report` is now the stronger surface: it reads the runtime-side
    parity stream and the latest parity report produced by `loom action execute`.
18. When available on the founder host, the parity stream also captures a real
    OpenClaw proof snapshot via `openclaw_runtime_proof.py --json`.
19. The decision surface still unions a local sanction preview derived from the
    resolved identity snapshot with the read-only reference gate result.
20. A fixture-backed rehearsal proves that `execute` / `remediation_only`
    restrictions deny locally even when the reference gate would otherwise allow.

## What the rehearsal does not prove

- It does not prove runtime-level contract compliance.
- It does not upgrade registry compliance beyond 0/7.
- It does not prove transport adapters exist.
- It does not prove OpenClaw replacement.
- It does not prove per-action OpenClaw parity.
- The live OpenClaw probe is a runtime health/proof snapshot, not a replayed
  gate-by-gate execution stream.
- The canonical kernel audit log is still not owned by Loom.

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

The current founder-host rehearsal now exercises both the old and new surfaces:

- `loom shadow compare` still compares reference-adapter event logs against
  Loom's shadow log for offline inspection
- `loom action execute` writes a runtime execution receipt, a runtime-side
  audit artifact, and a parity stream
- `loom parity report` surfaces that parity stream plus a live OpenClaw proof
  snapshot when the founder-host proof script is available, and now gives a
  guided next-step message when no parity artifacts exist yet

That is still not a claim of per-action runtime parity. It is a stronger,
runtime-side rehearsal surface than the previous file-only diff.

The rehearsal also emits `.loom/shadow/decision.json`, which records the
current gate outcome using the same reference stage and reason that drove the
preflight result. That decision artifact is still experimental and adapter-
backed; it does not make Loom a governed execution runtime.

There is now a separate allow-path rehearsal:

```bash
./scripts/rehearse_allow_execute.sh
```

That script proves the current local supervisor path:
- the effective decision is `allow`
- `loom action execute` dispatches the default Python worker
- the worker writes a result artifact under `.loom/runtime/jobs/<input_hash>/`
- runtime audit emission uses the kernel-owned `audit.py log-runtime` path
- parity artifacts include a per-action OpenClaw probe stream entry, even when
  the probe is unavailable in the synthetic fixture

The rehearsal now also proves both fail-closed surfaces against the current
kernel truth:

- `loom shadow enforce` returns exit code `2`
- `loom action execute` also returns exit code `2`

On the founder host, both deny because the reference budget gate denies the
action.

## Fixture-backed local sanction preview verification

The founder-host rehearsal proves the real current kernel path. A second,
fixture-backed rehearsal exists to prove the local sanction override path:

```bash
./scripts/rehearse_local_sanction_preview.sh
```

That script creates a synthetic kernel fixture where:

1. The resolved agent identity includes an `execute` restriction.
2. The read-only reference adapter still returns `allow`.
3. `loom shadow decide` reports `effective_source: local_sanction_preview`.
4. `loom shadow enforce` returns exit code `2`.
5. `loom action execute` also returns exit code `2` and writes a runtime
   execution receipt plus parity artifacts.

This is intentionally a fixture-backed proof surface, not a claim about the
founder host's current kernel state. The fixture rehearsal explicitly disables
the founder-host OpenClaw probe so the transcript stays synthetic. Its
transcript lives at
`examples/local-sanction-preview.txt`.
