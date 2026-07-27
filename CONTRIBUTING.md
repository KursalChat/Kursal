# Contributing to Kursal

Thanks for helping build Kursal. This document explains how we branch, commit, and ship.

## One-time setup

You need to have [cargo](https://rust-lang.org/tools/install/) and [bun](https://bun.sh) installed.

```bash
# you can inspect the justfile for more details of what this does
just install-dev-tools
```

## Branching model (git-flow)

| Branch          | Purpose                                              | Branch off | Merges into    |
| --------------- | ---------------------------------------------------- | ---------- | -------------- |
| `main`          | Production. Tagged releases only. Always shippable.  | -          | -              |
| `dev`           | Integration. All everyday work lands here.           | -          | -              |
| `feature/*`     | One feature or fix each.                             | `dev`      | `dev` (squash) |
| `release/X.Y.Z` | Stabilize a release: version bump, changelog, fixes. | `dev`      | `main` + `dev` |
| `hotfix/X.Y.Z`  | Urgent fix to live code.                             | `main`     | `main` + `dev` |

### Everyday flow (a normal change)

```bash
git checkout dev && git pull
git checkout -b feature/my-thing
# ... work, commit using the commit format ...
git push -u origin feature/my-thing
# open a PR into dev, let CI pass, squash-merge
```

`release/*` and `hotfix/*` flows live in [RELEASING.md](RELEASING.md).

## Commit format

`type: subject` - full rules and examples in [COMMITS.md](COMMITS.md). Enforced locally (git hook) and in CI.

## Merge strategy

- `feature/*` → `dev`: **squash** (one clean commit per feature; keeps `dev` readable).
- `release/*` and `hotfix/*` → `main`: **merge commit** (`--no-ff`), then tag.
- After every release/hotfix: back-merge `main` → `dev`.

## Pull requests

- Target `dev` (features/fixes) or `main` (release/hotfix only).
- CI (`checks`) must be green: frontend type-check + tests + translations, Rust fmt + clippy, and commitlint.
- Fill in the PR template.

## Versioning

[SemVer](https://semver.org/). We stay on `0.x` through development.
