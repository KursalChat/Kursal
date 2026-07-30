set shell := ["bash", "-uc"]

version := `./bin/version.sh`
prerelease := if version =~ "-" { "--prerelease" } else { "" }
website := "~/Code/Kursal-Website/static"
homebrew := "~/Code/homebrew-kursal/Casks/kursal.rb"

# list recipes
default:
    @just --list

# --- dev tooling ---

dev id="0":
    RUST_LOG=info cargo tauri dev -- -- --database-id="{{ id }}" --unsafe-write-key-to-file

install-dev-tools:
    install-hooks
    cargo install tauri-cli --version "^2.0.0" --locked
    cd kursal-tauri && bun install --frozen-lockfile && cd ..

install-hooks:
    git config core.hooksPath .githooks
    chmod +x .githooks/pre-push .githooks/commit-msg
    echo "✓ git hooks installed"

# --- release ---

# ORCHESTRATION: `just cut <v>` (prepare+build, local) -> test -> `just ship <v>`.
# See RELEASING.md

# prepare a stable/beta release: branch from dev, bump+tag, build
cut v:
    ./bin/cut.sh {{ v }}

# start a hotfix branch off main
cut-hotfix v:
    ./bin/cut-hotfix.sh {{ v }}

# publish a prepared release from its branch
ship v:
    ./bin/ship.sh {{ v }}

# bump version + changelog + commit + tag (run on a release/* or hotfix/* branch)
release v: verify
    bun run bin/bump-version.ts {{ v }}
    git cliff --tag v{{ v }} -o CHANGELOG.md
    git add Cargo.toml kursal-tauri/package.json CHANGELOG.md
    git commit -m "chore(release): v{{ v }}"
    git tag -a v{{ v }} -m "v{{ v }}"
    @echo "Tagged v{{ v }}. Next: merge into main + dev, then 'just build' and 'just publish'."

# --- build & publish ---
build: clean build-win build-mac build-linux build-android build-ios build-relay gen-manifest

clean:
    rm -f build/Kursal*
    rm -f build/latest*.json

    mkdir -p build

build-opus:
    ./bin/build-opus.sh

build-abseil:
    ./bin/build-abseil.sh

build-frontend:
    cd kursal-tauri && bun install --frozen-lockfile && bun run build

build-win: build-opus build-frontend
    ./bin/build-win.sh

build-mac: build-opus build-abseil build-frontend
    ./bin/build-mac.sh

build-linux: build-frontend
    ./bin/build-linux.sh

build-android: build-opus build-frontend
    ./bin/build-android.sh

build-ios: build-opus build-frontend
    echo "TODO: build iOS (waiting for paid apple cert)"
    # ./bin/build-ios.sh

gen-manifest:
    ./bin/gen-manifest.sh

preflight:
    ./bin/preflight.sh

build-relay:
    ./bin/build-relay.sh

publish: preflight publish-github publish-relay publish-beta-manifest publish-api-docs publish-homebrew

publish-beta: preflight publish-github publish-relay publish-beta-manifest

publish-github:
	gh release create v{{ version }} --verify-tag {{ prerelease }} --title "v{{ version }}" --notes "$(git cliff --latest --strip all)"
	for f in ./build/*; do echo "Uploading: $f"; gh release upload v{{ version }} "$f"; done

publish-relay:
    ./bin/build-relay.sh --push
    gh release upload v{{ version }} ./dist/kursal-relay-{{ version }}-linux-*.tar.gz ./dist/kursal-relay-{{ version }}-linux-*.tar.gz.sha256 --clobber

publish-beta-manifest:
    gh release upload beta ./build/latest.json --clobber

publish-api-docs:
    cargo run -p kursal-core --bin gen_api_docs -- --out {{ website }}/api/openapi.json

publish-homebrew:
    ./bin/publish-homebrew.sh {{ version }} {{ homebrew }}

# --- format ---
format: format-frontend format-rust

format-frontend:
    cd kursal-tauri && bun run format

format-rust:
    cargo fmt -p kursal-cli -p kursal-core -p kursal-app

# --- checks ---
verify: check-rust (check-frontend "--strict")

check strict="": format check-rust (check-frontend strict)

check-frontend strict="":
    cd kursal-tauri && bun run check
    cd kursal-tauri && bun run test
    bun ./bin/checkTranslations.ts {{ strict }}

check-rust:
    cargo fmt -p kursal-cli -p kursal-core -p kursal-app -- --check
    cargo clippy -p kursal-cli -p kursal-core -p kursal-app --all-targets --no-deps -- -D warnings
    cargo clippy -p kursal-core --no-default-features --all-targets --no-deps -- -D warnings
    cargo test -p kursal-cli -p kursal-core -p kursal-app

check-mobile: check-android check-ios

check-android:
    #!/usr/bin/env bash
    set -euo pipefail
    source "{{ justfile_directory() }}/bin/toolchain.sh" android
    cargo clippy --target aarch64-linux-android -p kursal-core -p kursal-app --no-deps -- -D warnings

check-ios:
    #!/usr/bin/env bash
    set -euo pipefail
    source "{{ justfile_directory() }}/bin/toolchain.sh" ios
    cargo clippy --target aarch64-apple-ios -p kursal-core -p kursal-app --no-deps -- -D warnings

check-translation:
    bun ./bin/checkTranslations.ts
