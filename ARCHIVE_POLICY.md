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
