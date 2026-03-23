<p align="center">
  <img src="assets/meridian_loom_lockup.svg" alt="Meridian Loom" width="720">
</p>

# Meridian Loom Publication Checklist

This checklist now records the conditions that were required for the first
public publication of the experimental Loom scaffold. It remains deliberately
strict so the repo does not overclaim maturity.

## Local repository truth

- [x] `README.md` explicitly says this is an experimental scaffold
- [x] `LICENSE` is present
- [x] docs hero assets exist (`meridian_loom_lockup.svg`, `loom_runtime_panels.svg`)
- [x] GitHub Actions workflow exists for `cargo test` and `cargo build`
- [x] `loom.toml.example` exists
- [x] Setup rehearsal script exists
- [x] Runtime rehearsal surface exists (`loom action execute` + `loom parity report`)
- [x] Setup rehearsal transcript is committed
- [x] Cargo workspace builds locally
- [x] Cargo workspace tests pass locally

## First public push status

- [x] GitHub API access works again
- [x] Public repository is created
- [x] Remote `origin` points at the public repository
- [x] First push succeeds

## First public release message

The first public publication must preserve these truths:

- Loom is not a runtime yet
- contract compliance remains 0/7 proven hooks
- OpenClaw remains the live runtime today
- this scaffold exists for CLI/setup/operator rehearsal, not runtime claims

## Recommended local command

```bash
./scripts/check_publication_readiness.sh
```
