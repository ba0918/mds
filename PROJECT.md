# Project Context

## What this is

`mds` is a CLI that validates Markdown documents against a YAML schema declared in the
document's frontmatter (`$schema`) and extracts structured values from them. It exists so that
the format of LLM-written Markdown can be pinned down mechanically. ADR is the first
application. The specification is `docs/spec/mds.md`.

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

## Conventions specific to this project

Language rules:

- User-visible CLI text (help, errors, output) is English.
- Documentation (specifications, plans, decision records) is Japanese.
- Code comments are Japanese.
- README, `AGENTS.md`, and `PROJECT.md` are English.

## Constraints

## Glossary

Domain terms whose meaning in this project differs from ordinary usage are in `CONTEXT.md`.
