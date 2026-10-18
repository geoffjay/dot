#!/usr/bin/env bash
# End-to-end smoke of the dot CLI in an isolated temp HOME.
# Mirrors crates/dot/tests/cli.rs but against the real built binary
# with multiple modules enabled.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

BIN="target/debug/dot"
if [[ ! -x "$BIN" ]]; then
    cargo build --workspace --quiet
fi

SMOKE_HOME="$(mktemp -d)"
trap 'rm -rf "$SMOKE_HOME"' EXIT

export HOME="$SMOKE_HOME"
unset XDG_STATE_HOME DOT_CONFIG 2>/dev/null || true

echo "== init =="
cat > "$SMOKE_HOME/answers.toml" <<'EOF'
modules = ["zsh", "git", "starship", "gh", "brew"]

[user]
name = "Smoke Tester"
email = "smoke@example.com"
theme = "nord"
EOF
"$BIN" init --non-interactive --answers "$SMOKE_HOME/answers.toml"

echo "== diff (expect differences, exit 1) =="
if "$BIN" diff; then
    echo "FAIL: diff should report differences" >&2
    exit 1
fi

echo "== write =="
"$BIN" write

for f in .zshrc .gitconfig .config/starship.toml .config/gh/config.yml .config/brew/Brewfile; do
    [[ -f "$SMOKE_HOME/$f" ]] || { echo "FAIL: missing $f" >&2; exit 1; }
done

grep -q "Smoke Tester" "$SMOKE_HOME/.gitconfig" || { echo "FAIL: name not templated" >&2; exit 1; }
grep -q "smoke@example.com" "$SMOKE_HOME/.gitconfig" || { echo "FAIL: email not templated" >&2; exit 1; }

echo "== diff (identical, exit 0) =="
"$BIN" diff

echo "== write (idempotent) =="
out=$("$BIN" write)
grep -q "0 created" <<<"$out" || { echo "FAIL: second write not idempotent: $out" >&2; exit 1; }

echo "== local edit + diff =="
echo "# local change" >> "$SMOKE_HOME/.zshrc"
if "$BIN" diff; then
    echo "FAIL: diff should report the local edit" >&2
    exit 1
fi

echo "== backup =="
"$BIN" backup

echo "== restore (refuse, then force) =="
echo "# changed after backup" >> "$SMOKE_HOME/.zshrc"
if "$BIN" restore; then
    echo "FAIL: restore should refuse changed file" >&2
    exit 1
fi
"$BIN" restore --force
grep -q "# local change" "$SMOKE_HOME/.zshrc" || { echo "FAIL: restore lost the snapshot" >&2; exit 1; }

echo "== propose dry-run =="
"$BIN" propose --dry-run | grep -q "local change" || { echo "FAIL: propose missed the drift" >&2; exit 1; }

echo "== disable module =="
sed -i.bak '/^\[modules.zsh\]$/,/^\[hosts\]$/ s/^enabled = true$/enabled = false/' "$SMOKE_HOME/.config/dot/config.toml"
rm "$SMOKE_HOME/.config/dot/config.toml.bak"
"$BIN" diff
echo "ALL SMOKE CHECKS PASSED"