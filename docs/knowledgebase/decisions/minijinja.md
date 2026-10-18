---
type: Decision
title: minijinja for template rendering
description: Engine choice, template context contract, and the has() tool probe.
tags: [decision, templates, minijinja, rendering]
timestamp: 2026-10-18
---

# D2 — minijinja

## Context

Templates need Jinja2-style conditionals to centralize the host-OS
guards that cfg duplicated inside every file.

## Decision

**minijinja** (autoescape off — shell config, not HTML). Template
context contract:

* `user.*` — name, email, theme from the config `[user]` table
* `host.*` — os, distro, arch, hostname, username, package_managers
* `module`, `dot_version` — identity strings
* `settings.*` — module defaults deep-merged with `[modules.<m>]`,
  `[hosts.<host>.modules.<m>]`, and CLI `--set` overrides
* `has("tool")` — PATH probe at write time (replaces cfg's
  `if [[ -x $(which bat) ]]` guards); cached per render

## Implementation notes (biting points already hit)

* `Environment::add_template` borrows for the environment's lifetime:
  collect owned template sources into a `Vec<(String, String)>` that
  outlives the env, then add. Overlay templates are collected first so
  they win name collisions.
* Functions added via `add_function` must be `Send + Sync` and `Fn`:
  the `has` cache is a `Mutex<BTreeMap<String, bool>>`, not a `RefCell`.
* `set_auto_escape_callback(|_| AutoEscape::None)` — the callback form
  takes a closure, not an `AutoEscape` value.
* Render is all-or-nothing per write: templates are collected and
  registered, then all files render, and only then does `plan::execute`
  touch the filesystem.