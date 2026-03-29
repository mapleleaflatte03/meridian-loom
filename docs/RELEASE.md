# Release

Loom releases are local-first operator packages published as GitHub release assets.

## Release layout

The tarball produced by `scripts/package_release.sh` contains:

- `bin/loom`
- `config/loom.toml.example`
- `docs/*.md`
- `scripts/install_local.sh`
- `scripts/release_local.sh`
- `scripts/package_release.sh`
- `scripts/acceptance_local_service.sh`
- `scripts/acceptance_container_service.sh`
- `scripts/verify_release_local.sh`
- `deploy/systemd/loom.service`
- `deploy/systemd/loom-user.service`
- `Dockerfile`
- `docker-compose.yml`
- `Makefile`
- `README.md`
- `manifest.txt`

## Build a release

```bash
./scripts/release_local.sh --kernel-path /opt/meridian-kernel
```

## Publish a tagged release

Create and push a tag like `v0.1.10`. The GitHub release workflow builds the package archive for the host runner, attaches:

- `meridian-loom-<version>-<os>-<arch>.tar.gz`
- `meridian-loom-<version>-<os>-<arch>.tar.gz.sha256`

and publishes them on the matching GitHub Release.

## Install a release

```bash
./scripts/install_local.sh dist/meridian-loom-*.tar.gz
```

## Validate a release

```bash
./scripts/verify_release_local.sh --kernel-path /opt/meridian-kernel
```

To force a fresh Docker image build during container verification:

```bash
./scripts/acceptance_container_service.sh --kernel-path /opt/meridian-kernel --build-image always
```

## systemd

Use the checked-in units as examples:

- `deploy/systemd/loom.service`
- `deploy/systemd/loom-user.service`

Both run Loom in foreground mode so systemd owns the process lifecycle.

## Truth boundary

A Loom release today is a production-oriented local runtime package. It is not
a hosted runtime release and it is not a legacy-runtime cutover artifact.
