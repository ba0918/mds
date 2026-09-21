# Project Context

## What this is

`mds` is a CLI that validates Markdown documents against a YAML schema declared in the
document's frontmatter (`$schema`) and extracts structured values from them. It exists so that
the format of LLM-written Markdown can be pinned down mechanically. ADR is the first
application.

## Specification

The specification lives in two places, and both are kept up to date:

- `docs/spec/mds.md` — the prose specification, written for people.
- `docs/ir/` — the same specification normalised into kotowari's IR form, so that it can be
  checked mechanically. Each IR document declares a schema from `.mds/schemas/` in its
  `$schema` frontmatter, so it passes both `kotowari check` and `mds check docs/ir`.

Rules the IR does not yet carry are recorded as gaps in `docs/ir/FLAGS.md`. Decisions the IR
cites as sources live in `docs/decision/records/`.

Read the `kotowari` skill before writing or revising an IR document.

## Stack and layout

Rust. Cargo workspace (monorepo):

- `crates/mds-core/` — the schema language, validation, and extraction. Kept free of CLI
  dependencies.
- `crates/mds-*/` — further crates as the tool grows.
- `src/main.rs` — the CLI entry point.

The core is kept separate from the CLI so that it can be reused, for example inside `kotowari`.

## Commands

| Purpose | Command |
|---|---|
| Build | `cargo build` |
| Test | `cargo test` |
| Lint | `cargo clippy` |
| Run locally | `cargo run -- <args>` |
| Quality gates (pre-commit) | `lefthook run pre-commit --no-auto-install` |
| Check the IR against its schemas | `cargo run -- check docs/ir` |
| Check the IR against the spec form | `kotowari check` |

lefthook runs the quality gates (fmt / clippy / test) on every `pre-commit`, as defined in
`lefthook.yml`. Do not run `lefthook install` on a machine whose global pre-commit hook already
delegates to `lefthook run pre-commit --no-auto-install` (as on the developer machines provisioned
with mise): installing would replace that global hook. Which case applies can be determined by
checking the file at `git rev-parse --git-path hooks`/pre-commit (the shared hooks directory, even
in a worktree) for a delegation to `lefthook run pre-commit --no-auto-install`.
On a machine without such a global hook, run `lefthook install` once to generate the local hooks.
With nothing staged, the gates are skipped and the run reports success, so run it with at least
one staged change to actually verify the gates.

## Conventions specific to this project

Language rules:

- User-visible CLI text (help, errors, output) is English.
- Documentation (specifications, plans, decision records) is Japanese.
- Code comments are Japanese.
- README, `AGENTS.md`, and `PROJECT.md` are English.

## Constraints

## Glossary

Domain terms whose meaning in this project differs from ordinary usage are in `CONTEXT.md`.
