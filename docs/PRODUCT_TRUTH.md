# Product Truth

Meridian Loom is production-oriented in local form.

That means:

- installable
- runnable as a local service
- operable through CLI and local HTTP control plane
- auditable through local proof, audit, and parity artifacts

It does **not** mean:

- hosted runtime replacement
- OpenClaw retirement
- live transport cutover
- full hosted parity
- full contract compliance across every future runtime hook

## True today

- local service lifecycle is real
- background and foreground service modes exist
- tokenized local HTTP control plane is real
- queue, scheduler, job ledger, logs, audit, and parity paths are real
- Docker, tarball, and source install flows exist
- operator docs are aligned with the current code surface

## Still bounded today

- local-first only
- single-node only
- no hosted scheduler or hosted audit backend
- live host still runs OpenClaw
- `phase3/minimal-gap-replay-slice` is archival and non-main; its remaining diff is superseded by `origin/main`

## Language discipline

Use:

- `local-first runtime`
- `production-oriented local service`
- `operator-ready local surface`

Avoid:

- `production replacement for OpenClaw`
- `hosted runtime`
- `transport cutover complete`
