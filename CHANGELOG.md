# Changelog

Notable changes to `mds`, newest first. Versions follow [Semantic Versioning](https://semver.org/).

Entries are written for someone upgrading from one release to the next: what changed, and what
that asks of you.

## Unreleased

## [0.1.0] - 2026-09-22

First release. `mds` validates a Markdown document against a YAML schema the document declares
itself, and hands its contents back as structured data.

### The command

- `mds check <file|dir>` validates a document, or every schema-declaring `.md` under a directory,
  and reports findings as text or JSON. Exit code 0 when clean, 1 when a document has findings,
  2 when the check could not run.
- `mds values <file>` extracts the declared values as JSON or as indented text.
- `mds ast <file>` prints the Markdown tree, with `--schema` for the typed form.
- `--open` relaxes closed-world validation for undeclared structures.

### The schema language

A schema declares the shape of a document: `title`, `preamble`, `sections`, `item`, `field`,
`statement`, `bullets`, `table`, `codeblock`. Every rule takes an occurrence count (`required`,
`repeat`), an `extract` naming where its value lands, and an optional `when` making it
conditional on another field's value.

Validation is closed-world by default: a heading or a line the schema never declared is an error.

Output keys come from the schema, not from the document. Renaming a heading changes the schema,
not the JSON your code reads.

### The library

`mds-core` is usable as a crate. Six entry points are the contract: `frontmatter_schema`,
`resolve_schema`, `parse_schema`, `Document::parse`, `validate`, `extract_values`. Reading files
and fetching URLs stays with the caller.

### Installing

```console
$ cargo install --git https://github.com/ba0918/mds
```

Requires Rust 1.89 or newer.

[0.1.0]: https://github.com/ba0918/mds/releases/tag/v0.1.0
