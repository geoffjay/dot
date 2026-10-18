---
type: Concept
title: dot commands and lifecycle
description: What each dot command does, exit codes, and how they compose into the manage/propose loop.
tags: [concept, cli, commands]
timestamp: 2026-10-18
---

# Commands and lifecycle

Every content-producing command accepts `--module <name>` (repeatable) to
scope it; without it, all enabled modules are acted on. `--config <path>`
or `$DOT_CONFIG` override the config location.

## Command summary

| command | effect | notable flags |
|---|---|---|
| `init` | wizard → `~/.config/dot/config.toml` | `--force`, `--non-interactive --answers <file>` |
| `diff` | preview of `dot write` | diff(1) exit codes: 0 identical, 1 differences, 2 error |
| `write` | render + install, record manifest | `--force` rewrites unchanged files |
| `backup` | snapshot managed files + config + manifest | `--keep N` (default 10) |
| `restore` | restore from a snapshot | `--backup <id>`, `--force`, `--list` |
| `update` | upgrade dot via its install channel | `--check` reports current vs. latest |
| `propose` | GitHub issue with diffs of local changes | `--dry-run` prints the payload |

## Lifecycle

`init` writes module entries in table form (`[modules.zsh] enabled =
true`) so settings can be added under the same table without a
structural conflict. `write` is all-or-nothing: every template renders
before any filesystem mutation; a render error aborts before touching
`$HOME`. Files unchanged since the last write (sha256 vs. manifest) are
skipped. Adopting an unmanaged file saves the original to
`backups/pre-write/` first.

`diff` renders the same plan and prints unified diffs; exit 1 means
"differences exist", making it scriptable (shell prompts, CI gates).

`backup` snapshots manifest-managed files into
`<state>/backups/<YYYYmmdd-HHMMSS>/`, prunes beyond `--keep`.
`restore` refuses files changed on disk after the snapshot unless
`--force`; `--list` shows ids newest-first.

`update` prefers the native channel: Homebrew when brew manages dot,
AUR helper (`yay`/`paru`) when pacman owns `/usr/bin/dot`, else manual
instructions. `--check` compares against the GitHub latest release.

`propose` diffs on-disk managed files against what the current binary +
config *would* render — that delta is exactly the local knowledge
upstream lacks. The issue body carries a host facts table, the
effective module settings with values of sensitive-looking keys
(token/password/secret/key) redacted, and fenced diffs. Files listed in
a module's `sensitive = [...]` never appear in propose output at all.

## Answer-sheet gotcha

In TOML, keys after a table header belong to that table. In an init
answers file, top-level `modules = [...]` must precede `[user]` or it
parses as `user.modules` and the selection is silently empty.