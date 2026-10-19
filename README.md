# dot

Modular, versioned dotfile manager. Renders per-user, per-host
configuration from versioned templates embedded in the binary, and
pushes local changes back upstream as GitHub issues.

## Why

The previous approach ([cfg](https://github.com/geoffjay/cfg)) distributed
dotfiles as a bare git checkout of `$HOME`. That made user preferences
impossible (content was verbatim), duplicated host-OS logic inside every
file, and updated via `git pull` into a dirty work tree — hosts drifted.

`dot` inverts the model: the binary owns the content (embedded modules),
config selects and parametrizes it, host facts feed the templates, and
`dot propose` is the explicit path back upstream.

## Install

**Homebrew (macOS / Linux):**

```sh
brew install geoffjay/tap/dot
```

**Arch Linux (AUR):**

```sh
yay -S dot-bin
```

**Prebuilt binaries:** download the tarball for your platform from the
[releases](https://github.com/geoffjay/dot/releases) page and verify
against `checksums.txt`.

## Usage

```sh
dot init      # interactive setup: write ~/.config/dot/config.toml
dot diff      # preview what dot write would change (diff(1) exit codes)
dot write     # render and install configuration for enabled modules
dot backup    # snapshot managed files into the state dir
dot restore   # restore managed files from a backup
dot update    # upgrade dot via its install channel (brew, AUR)
dot propose   # open a GitHub issue with diffs of local changes
```

`dot diff` exits like diff(1): 0 identical, 1 differences, 2 error —
use it in scripts and prompts.

`dot init` is scriptable: `dot init --non-interactive --answers
answers.toml`, with `modules = [...]` at the **top** of the file (in
TOML, keys after a `[user]` header belong to that table).

Run `dot <command> --help` for flags. Common flags: `--module <name>`
(scope a command to modules, repeatable), `--config <path>` (override
the config file).

## Configuration

`~/.config/dot/config.toml` (legacy `~/.dotrc` is honored as a fallback):

```toml
version = 1

[user]
name = "Geoff Johnson"
email = "geoff@example.com"
theme = "nord"

[modules.git]
enabled = true

[modules.zsh]
enabled = true
plugins = ["autosuggestions", "syntax-highlighting"]

[hosts.worktop.modules.zsh] # per-host overrides, keyed by hostname
plugins = ["autosuggestions"]
```

Values merge module defaults → `[modules.<m>]` →
`[hosts.<host>.modules.<m>]` → CLI `--set key=value`. Disabling a module
is `enabled = false` — opt-out per user or per host.

## Modules

Each module (`modules/<name>/`) is one tool: `module.toml` metadata
(description, supported OSes, defaults, target files) plus Jinja
templates rendered with `user.*`, `host.*` (os, distro, arch, hostname,
package managers) and a `has("tool")` helper — replacing the hand-coded
`if [[ -x $(which bat) ]]` guards of the old setup. Modules are embedded
in the binary at build time, so content and tool version never drift.

Shipped modules: bash, brew, gh, ghostty, git, htop, kitty, lsd,
starship, tmux, zsh. The zsh module is ingested from a real host and
covers the full shell setup: `.zshrc`, `.zprofile`, `.zshenv`,
`.zshenv.d/`, `.zshrc.d/` fragments, `~/.zsh/util/` scripts and
plugins — rendered byte-identically to the source host. To override a
template locally without waiting for a release, drop a file in
`~/.config/dot/modules/<name>/templates/` — the overlay wins at render
time and is the natural input to a `propose`.

`dot propose --module zsh` diffs on-disk managed files against what the
current binary would render, redacts sensitive values, excludes files a
module marks sensitive (e.g. gh's `hosts.yml`), and opens a labeled
GitHub issue — `--dry-run` prints the payload.

## State

Managed files, backups, and the manifest live under
`~/.config/state/dot` (`${XDG_STATE_HOME:-~/.local/state}/dot` on
Linux). Everything dot writes is recorded in `manifest.toml`; files dot
does not own are never touched, and adopting one backs up the original
to `backups/pre-write/` first.

## Development

```sh
cargo make lint   # fmt + clippy
cargo make test   # unit + CLI integration
cargo make e2e    # end-to-end smoke in a temp HOME
```

Licensed under MIT OR Apache-2.0.