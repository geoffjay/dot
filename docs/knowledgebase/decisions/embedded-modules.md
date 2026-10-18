---
type: Decision
title: embed modules in the binary; manifest defines ownership
description: Why dot embeds module content at build time and tracks managed files in a manifest under the XDG state dir.
tags: [decision, modules, include_dir, manifest, state]
timestamp: 2026-10-18
---

# D1/D4 — embedded modules and manifest-based ownership

## Context

The cfg bare-repo model distributed files via `git pull` into `$HOME`;
content and tool had no version relationship and nothing recorded
which files were "managed".

## Decision

1. Module content lives in `modules/<name>/` and is **embedded into the
   binary** at build time via `include_dir`. The rendered output is a
   pure function of (binary, config, host facts) — content and tool
   version are inseparable, so drift between "what dot thinks" and
   "what dot is" is impossible by construction. Local changes travel
   upstream via `dot propose`; the escape hatch for unmerged needs is a
   template overlay in `~/.config/dot/modules/<name>/templates/`.
2. Every dot-written file is recorded in `manifest.toml` under the XDG
   state dir (`~/.local/state/dot` on Linux, `~/.config/state/dot` on
   macOS — no standard macOS state location exists). "Managed" is an
   explicit, inspectable list; files not in the manifest are never
   touched except by an explicit adoption that backs up the original
   first.

## include_dir 0.7 gotcha

`Dir::get_file`/`get_dir` on a subdirectory resolve paths relative to
the *embedded root*, not the subdirectory: `MODULES.get_file("zsh/
templates/zshrc.tmpl")` works, `subdir.get_file("zshrc.tmpl")` does
not. All lookups in `dot-core::module` use root-relative paths.

## Consequences

* `include_dir` paths are root-relative — document for anyone adding
  lookups.
* Target paths are `$HOME`-only (enforced by path normalization in
  `render::resolve_target`); system files stay with the OS package
  manager.
* macOS state path is a dot convention (`~/.config/state/dot`),
  documented in the README and `state.rs`.