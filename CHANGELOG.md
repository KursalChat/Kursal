# Changelog

All notable changes to Kursal are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).
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
