# Meridian Loom Publication Checklist

This checklist is for the first public publication of the experimental Loom
scaffold. It is deliberately strict so the repo does not overclaim maturity on
day one.

## Local repository truth

- [x] `README.md` explicitly says this is an experimental scaffold
- [x] `LICENSE` is present
- [x] GitHub Actions workflow exists for `cargo test` and `cargo build`
- [x] `loom.toml.example` exists
- [x] Setup rehearsal script exists
- [x] Setup rehearsal transcript is committed
- [x] Cargo workspace builds locally
- [x] Cargo workspace tests pass locally

## Required before first public push

- [ ] GitHub CLI authentication works again
- [ ] Public repository is created
- [ ] Remote `origin` points at the public repository
- [ ] First push succeeds

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
