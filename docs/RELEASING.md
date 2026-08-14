# Releasing Kursal

Releases are **built on GitHub Actions**, every platform in parallel, and published from here. The whole flow is two commands:

1. `just cut <version>` (prepare + tag + push, which starts the build)
2. then `just ship <version>` (publish what CI produced)

The lower-level recipes (`release`, `build`, `publish`, …) still work standalone if you want to drive a step by hand.

`just build` still builds everything locally. It is the fallback when CI is unavailable.

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

Those variables needs to be defined in Github secrets: `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `ANDROID_KEYSTORE_B64` and `ANDROID_KEYSTORE_PASSWORD`.

## Stable release

```bash
just cut 0.2.0     # branch release/0.2.0 from dev, verify+bump+tag, push. CI builds.

# → watch it: gh run watch
# → grab what you want from the draft release and make sure it's good ←

just ship 0.2.0    # PR→main, un-draft + publish, back-merge→dev, clean up
```

## Beta / rc release

Betas live on the `release/<minor>` branch; nothing touches `main` until the stable ships. `cut`/`ship` detect the `-beta.N` suffix and skip the main-merge.

```bash
just cut 0.3.0-beta.1     # creates release/0.3.0 (reused by later betas)
# → test ←
just ship 0.3.0-beta.1    # un-draft the pre-release + beta manifest slot (confirms first)

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

just release 0.2.1                                  # verify + bump + changelog + tag
git push --follow-tags origin hotfix/0.2.1          # starts the CI build
just ship 0.2.1           # same confirm-gated merge→main + publish + back-merge→dev
```
