# Product Truth

Meridian Loom is production-oriented in local form. It is installable,
runnable as a local service, and auditable through local proof, audit, and
parity artifacts.

Current truth:

- local service lifecycle is real
- background and foreground service modes exist
- tokenized local HTTP control plane is real
- queue, scheduler, job ledger, logs, audit, and parity paths are real
- Docker, tarball, and source install flows exist
- operator docs are aligned with the current code surface

What it is not yet:

- hosted runtime replacement
- OpenClaw retirement
- live transport cutover
- full hosted parity
- full contract compliance across every future runtime hook

## Migration / cutover path

1. local service, proof, audit, and parity surfaces stay the source of truth
2. capability readiness and cutover checks are used to prove a request can move
   safely through the Loom surface
3. explicit owner authorization gates any hosted cutover attempt
4. live transport cutover and OpenClaw retirement are only valid after the
   hosted replacement path is fully proven

## Why OpenClaw still exists

OpenClaw still runs the live host because Loom is not yet the hosted runtime
replacement. The current code proves the local service boundary, capability
readiness, and parity surfaces, but it does not yet prove live transport cutover
or retirement of the existing host.

## Still bounded today

- local-first only
- single-node only
- no hosted scheduler or hosted audit backend
- live host still runs OpenClaw

## Language discipline

Use:

- `local-first runtime`
- `production-oriented local service`
- `operator-ready local surface`

Avoid:

- `production replacement for OpenClaw`
- `hosted runtime`
- `transport cutover complete`
