---
okf_version: "0.2"
---

# dot knowledge base

This is the working knowledge base for the dot project, conforming to the
[Open Knowledge Format (OKF) v0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md).

It consolidates working knowledge about the project: what dot is, how it is
structured, decisions and their rationale, recurring patterns, and plans.
It is authored by people and agents and meant to be read by both.

## For agents (policy)

This section is the single source of truth for how agents should use this knowledge
base. Tooling injects it into context automatically, so it does not depend on
`CLAUDE.md`/`AGENTS.md` being picked up.

Wired agents: oh-my-pi via the
`.omp/extensions/kb-hooks.ts` extension (injects this index at session start,
nudges after KB-relevant edits, and reminds once at session stop).

**Consult before acting.** Before working on a task, scan the entries below and
read any concept/decision/pattern doc relevant to what you are about to change.
Prefer the recorded decision or pattern over re-deriving one. This index is the
map; read the specific doc on demand rather than guessing.

**Update after acting.** Update the knowledge base when a change would make an
existing entry wrong or leave a new fact unrecorded. In particular:

* A new architectural decision or a change to startup/threading →
  add or update a [decision](decisions/index.md) and relevant concept docs.
* A new recurring convention → add a [pattern](patterns/index.md).
* A new concept or architectural understanding → add a [concept](concepts/index.md).
* A forward-looking plan or roadmap item → add a [plan](plans/index.md).
* An external source or spec referenced by the KB → add a [reference](references/index.md).

Concept docs require YAML frontmatter with a `type` field; `index.md` and
`log.md` are reserved. When you add a doc, add a one-line pointer to the matching
category index below and a line to [`log.md`](log.md). If you deliberately decide
*not* to record a change, that is fine — the policy is judgement, not a mandate
to touch the KB on every edit.

## Concepts

* [Commands and lifecycle](concepts/commands.md) - what each dot command does, exit codes, and how they compose into the manage/propose loop.

## Decisions

* [Embedded modules and manifest ownership](decisions/embedded-modules.md) - module content ships inside the binary; a manifest under the XDG state dir defines which files dot owns.
* [minijinja for template rendering](decisions/minijinja.md) - engine choice, template context contract, and the `has()` PATH probe.

## Patterns

* _(empty — add pattern docs here)_

## Plans
* [dot project](plans/dot-project.md) - requirements analysis and build plan for the dot CLI: modular, versioned dotfile management with per-user config, host facts, safe writes, and an upstream propose loop.

## References

* [OKF spec](references/okf-spec.md) - pointer to the Open Knowledge Format v0.2 specification.
