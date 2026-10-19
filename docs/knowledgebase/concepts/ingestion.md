---
type: Concept
title: ingesting host configuration into modules
description: The copy-verbatim-then-verify method for turning a live host's config files into byte-exact module templates.
tags: [concept, ingestion, modules, verification]
timestamp: 2026-10-18
---

# Ingesting host configuration into modules

## Goal

After ingestion, `dot diff` on the source host must report **zero**
changes: every rendered file byte-identical to the live file. Anything
else means the first `dot write` would silently rewrite working config,
and `propose` would forever report phantom drift.

## Method

1. **Copy verbatim.** `cp` every live file into `modules/<name>/templates/`
   as-is. Do not reformat, re-indent, or "improve" anything.
2. **Template only machine-bound values.** Absolute paths, user names,
   sizes that vary per host become `{{ settings["key"] }}` with the
   source host's value in `[defaults]`. Use absolute paths in defaults
   (`/Users/geoff/...`, not `~`) — substitution happens inside quotes
   where shell expansion never occurs. Keep `$HOME`-relative forms
   verbatim when the live file uses them.
3. **Guard only unconditional evals.** A line like
   `eval "$(yarn global bin)"` that runs at shell startup gets
   `{%+ if has("yarn") %}...{%+ endif %}`. Lines already runtime-guarded
   (`[[ -x $(which bat) ]]`, `[[ -d ... ]]`) stay untouched — dot guards
   on top of them are redundant and break byte-exactness.
4. **No `# Managed by dot` headers in ingested files.** Ownership is the
   manifest's job; headers would change every ingested file's bytes.
5. **Verify byte-exactness before committing.**
   `cargo run -p dot-core --example verify_zsh` renders the module
   against a temp HOME and compares every file to the live host,
   classifying diffs as intended vs unexpected; it must print
   `N byte-identical, 0 intended diffs` (adapt the example's module
   name/intended list per module).

## Whitespace control (Jinja)

* `env.set_keep_trailing_newline(true)` — without it every render loses
  the final newline and differs from source. Regression-tested in
  `render::tests::render_preserves_trailing_newline`.
* Standalone `{% if %}` lines insert newlines; use `{%+ if %}` / `{%+ endif %}`
  (or `{% if ... -%}` to trim) so the true-case output keeps the live
  file's exact bytes. Verify with the example, not by eye.

## TOML gotchas reused from init

* In `module.toml`, template names are flat keys
  (`"zshrc.d-10-alias.tmpl" = "~/.zshrc.d/10-alias"`), not nested paths.
* Runtime-generated files (e.g. `~/.zsh/completions/_pitchfork`) are
  not ingested — they are outputs, not configuration.