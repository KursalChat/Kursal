# Commit format

Kursal uses [Conventional Commits](https://www.conventionalcommits.org/). Keep it simple:

```
type: short summary in imperative mood
```

## Types

| type       | when to use                    |
| ---------- | ------------------------------ |
| `feat`     | new feature                    |
| `fix`      | bug fix                        |
| `docs`     | documentation only             |
| `style`    | formatting, no code change     |
| `refactor` | code change, no feature or fix |
| `perf`     | performance improvement        |
| `test`     | adding or fixing tests         |
| `build`    | build system or dependencies   |
| `ci`       | CI configuration               |
| `chore`    | misc maintenance               |
| `revert`   | undo a previous commit         |

## Rules

- lowercase `type`, then `: `, then the subject
- subject ≤ 72 chars, no trailing period, imperative (`add`, not `added`)
- breaking change: add `!` after the type (`feat!: ...`)

A `commit-msg` git hook checks this locally; CI checks it again on every PR. See [CONTRIBUTING.md](../CONTRIBUTING.md) to activate the hook.
