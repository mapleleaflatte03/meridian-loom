# Meridian Loom Mirror Archive Policy

This repository is a **mirror** of the canonical module at:

- https://github.com/mapleleaflatte03/meridian/tree/main/loom

## Policy

- Treat this repo as **read-only for product evolution**.
- Open all new issues in the monorepo:
  - https://github.com/mapleleaflatte03/meridian/issues
- Open all new PRs against the monorepo path `loom/`.
- Security reports should follow monorepo security process:
  - https://github.com/mapleleaflatte03/meridian/security/policy

## Allowed Mirror Updates

- Sync commits from monorepo -> mirror.
- Emergency metadata fixes that unblock users from reaching monorepo.

## Not Allowed Here

- New features
- New roadmap planning
- Non-sync refactors
- Divergent fixes that do not land in monorepo first

## Maintainer Checklist

1. Confirm `meridian` monorepo contains the intended change first.
2. Link every mirror sync commit to the monorepo commit/hash.
3. Keep issue templates and PR template redirecting to monorepo.
4. Keep README top banner pointing to canonical monorepo path.

## Archive Lock Checklist (Final Pass)

- [ ] GitHub repo description starts with: `[MIRROR - READ ONLY]`.
- [ ] GitHub repo homepage points to: `https://github.com/mapleleaflatte03/meridian`.
- [ ] `README.md` top section states this repo is a mirror for `meridian/loom`.
- [ ] `.github/ISSUE_TEMPLATE/config.yml` has `blank_issues_enabled: false`.
- [ ] All issue templates redirect users to monorepo issues/discussions/security links.
- [ ] `.github/pull_request_template.md` redirects PR authors to monorepo path `loom/`.
- [ ] Branch protection on `main` blocks direct pushes for non-maintainers.
- [ ] Mirror updates are sync-only from monorepo commits (no feature work here).
- [ ] Optional hard lock: archive this repo in GitHub UI after redirects are confirmed.

### Completion Gate

Mirror is considered closed when every checkbox above is done and any new issue/PR created in this repo is immediately redirected to the monorepo.
