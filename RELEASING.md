# Releasing Kursal

Releases are **built and published locally** (no cloud builds, no cloud signing). The whole flow is two commands — `just cut <version>` (prepare + build, all local) then `just ship <version>` (publish) — and `ship` stops for a typed `y/N` before every irreversible step (pushing `main`, publishing). The lower-level recipes it calls (`release`, `build`, `publish`, …) still work standalone if you want to drive a step by hand.

## Prerequisites

- [git-cliff](https://git-cliff.org/docs/installation/) (changelog generator)
- [GitHub CLI](https://cli.github.com/) (`gh`), authenticated with release write access

Distribution targets are local sibling repos (see the `justfile`):
`~/Code/Kursal-Website/static` (api docs) and `~/Code/homebrew-kursal` (cask).

One-time setup — create the permanent beta manifest slot (see [Update channels](#update-channels)):

```bash
gh release create beta --prerelease --title "beta channel manifest" \
  --notes "Updater manifest slot for the beta channel. Do not delete :)"
```

## Stable release

```bash
just cut 0.2.0     # branch release/0.2.0 from dev, verify+bump+tag, build. all local.

# → install/run the build from build/ and make sure it's good ←

just ship 0.2.0    # merge→main, push, publish, back-merge→dev, clean up
```

`just ship` confirms twice: once before `git push --follow-tags origin main`, once
before `just publish`.

## Beta / rc release

Betas live on the `release/<minor>` branch; nothing touches `main` until the stable
ships. `cut`/`ship` detect the `-beta.N` suffix and skip the main-merge.

```bash
just cut 0.3.0-beta.1     # creates release/0.3.0 (reused by later betas)
# → test ←
just ship 0.3.0-beta.1    # push branch + publish-beta (confirms first)

# more betas on the same branch:
just cut 0.3.0-beta.2 && just ship 0.3.0-beta.2

# finalize: ship the stable off the same branch
just cut 0.3.0 && just ship 0.3.0
```

## Hotfix (urgent fix to a live release)

The fix is manual, so this one isn't a single `cut`:

```bash
just cut-hotfix 0.2.1     # branch hotfix/0.2.1 off main

# ... fix the bug, commit ...

just release 0.2.1 && just build
just ship 0.2.1           # same confirm-gated merge→main + publish + back-merge→dev
```
