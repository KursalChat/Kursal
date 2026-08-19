# Changelog

All notable changes to Kursal are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

---

## [0.1.0-beta.9] - 2026-08-19

### Features

- New icon! well just has a background now
- Unlimited file size (+ ask to save)

### Bug Fixes

- Remove useless /chat page
- Pending UI state shows on intentional dial only
- Offline profile sharing waits for contact being online (large avatars)
- Desktop phone loadout drag
- Transparent avatars
- UI avatar updating everywhere
- Defer android permissions prompt
- Toggle camera mirroring
- Size issue + store self-avatar on disk
- Language loading improvement (+ coverage %)
- Small UI fixes + deps bump
- Clearing logs
- Avatar fallback
- Cooldown presence dial loop

### CI

- Pre-commit check-format
- Pre-commit formatting
- Autodownload node modules & default relay config

### Styling

- Using oxfmt instead of prettier

### Miscellaneous

- Kursal is now on OpenCollective!
- Remove migration code

## [0.1.0-beta.8] - 2026-08-14

### Bug Fixes

- Svelte #each unique issues
- Better UI update notifier
- UI bugs
- Include webview in RAM usage
- Listening port applied
- Bootstrap & kad fixes
- Stats on the dashboard improvements
- Expose health port on relay (4892)
- Less active connections
- Minor UI improvements & dependency bump
- UI mobile keyboard flicker
- Better QR code scanning for OTPs
- Bluetooth speed & reliability

### Performance

- Split rust using feature cfg
- Front general optimizations
- Store avatars as files
- Emoji rebuild (less disk usage)
- Remove DB mutex, batch commits and cache
- Faster OTP mining without compromising security

### Refactor

- Dashboard stats state on mobile

### Documentation

- Clarify RELAY hosting (health port and community)
- Some docs in docs folder

### CI

- Allow longer commits bodies for dependabot...
- Cache on dev branch

### Miscellaneous

- Changelog
- Changelog
- Backmerge release/0.1.0 into dev
- Updated lockfile
- Backmerge release/0.1.0 into dev

## [0.1.0-beta.6] - 2026-08-09

### Features

- Contact page complete rework
- Call ringtone (probably temporary)

### Bug Fixes

- Move conversation state to encrypted storage
- Optimizing final build size
- No unwraps in external bluetooth crate
- Smaller offline messages checks
- Addresses display, general UI improvements
- Prefer local connections & improved file transfers
- MacOS .kursal file association
- Contact order by last message time
- Possible database deadlocks

### Refactor

- Less code unwraps/expects

### CI

- Full publish (api-docs, brew)
- Tauri executable path
- Build releases on github actions

### Styling

- Format

### Miscellaneous

- Small translation fixes (2)
- Small translation fixes
- Translated French (100%)
- Backmerge release/0.1.0 into dev

## [0.1.0-beta.5] - 2026-08-05

### Features

- "offline" LTC access for PeerID rotation
- Updater progress bar

### Bug Fixes

- OTP & LTC deadlock
- Pinned messages & unread improvements
- More accurate user status

### Styling

- Reworked comments

## [0.1.0-beta.4] - 2026-08-04

### Features

- Major LTC improvements / configuration

### Bug Fixes

- En/fr grammar & consistency
- Mobile edge-to-edge display
- Re-enable mdns
- Build ubuntu 22 for more compatibility
- ToS pop up & autostart permission

### CI

- Build deps cleanups
- Orb start for publishing relay
- Preflight signature
- Adjustements

### Styling

- Changelog line breaks

### Revert

- No need to restore last contact

## [0.1.0-beta.3] - 2026-08-01

### Bug Fixes

- File download path in APPDATA instead of CACHE
- Version pinning & executable name
- Simpler DHT pow + verification
- Blocked chat could send messages
- Keychain on linux uses dbus now
- Update screen & user input
- More commands in the control menu
- Auto notifications removal
- Cliff tag order

### CI

- Vulnerability + useless code
- Release scripts hotfix (#2)
- Check on pull request
- Dont build ios

### Miscellaneous

- Merge french translations #6
- Add french translations
- Clippy
- Ignore iOS build for now

### Revert

- Don't fully close the window, keep it open and hide

## [0.1.0-beta.2] - 2026-07-30

### Features

- Mobile native file sharing (#4)
- Large messages send as file (10k+)
- In-app log view
- File caption (sends as msg)

### Bug Fixes

- Remove unused french keys
- Otp consumption
- Offline messages edits
- Audio & video jitter
- Message action menu
- App focus notifications
- Contact onboarding mobile self-lock
- File upload location & UI
- Less in-app notifications
- File downloads
- Webrtc-audio-processing breaks if no brew

### Miscellaneous

- Clippy

## [0.1.0-beta] - 2026-07-27

### CI

- Pre-push fallback
- Check splits
- Ignore bots in commitlint
- Checkout & commitlint fix
- Fail missing non-english translations

### Miscellaneous

- Bump actions/checkout in the actions group
- Initial commit

