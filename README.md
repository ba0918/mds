# mds

Validate Markdown documents against a YAML schema declared in their own frontmatter, and
extract structured values from them.

LLM-written Markdown drifts: a heading is renamed, a field line disappears, a table grows a
column. `mds` lets a document declare the shape it promises to keep, checks it mechanically, and
hands the contents back as JSON so that the rest of your tooling never has to parse Markdown.

## Install

```console
$ cargo install --git https://github.com/ba0918/mds
```

## Quick start

A document points at its schema from the frontmatter:

```markdown
---
$schema: ../../.mds/schemas/adr.yaml
---
# ADR-0001: Use a YAML schema

- Status: accepted
- Date: 2026-09-21

## Context

The format of generated documents drifts.

## Decision

Declare the format and check it.
```

The schema declares the shape and what to extract:

```yaml
name: adr
document:
  title:
    pattern: "^ADR-(?<id>\\d{4}):"
    extract: { path: id, group: id }
  preamble:
    fields:
      - name: Status
        enum: [accepted, rejected, proposed]
        extract: status
      - name: Date
        pattern: "^\\d{4}-\\d{2}-\\d{2}"
        extract: date
  sections:
    - name: Context
      extract: sections.context
      statement: { required: false }
    - name: Decision
      extract: sections.decision
      statement: { required: false }
```

Then:

```console
$ mds check docs/adr/            # validate every schema-declaring document under a directory
$ mds values docs/adr/0001.md --format json
{
  "id": "0001",
  "status": "accepted",
  "date": "2026-09-21",
  "sections": { "context": "The format of generated documents drifts.",
                "decision": "Declare the format and check it." }
}
```

## The schema language

| Rule | Validates | Can hold |
|---|---|---|
| `title` | the `#` heading | a pattern, a named-group capture |
| `preamble` | everything between the title and the first `##` | fields, statements, bullets, a table, a code block |
| `sections` | `##` headings | fields, statements, bullets, a table, a code block, items |
| `item` | `### ID: Name` headings | fields, statements, bullets, a table, a code block |
| `field` | `- Name: value` lines | a pattern, an enum, a separator, a condition |
| `statement` | prose paragraphs | a pattern, an enum, a condition |
| `bullets` | list lines that are not fields | a pattern, nested children |
| `table` | Markdown tables | an expected header, or none |
| `codeblock` | fenced blocks | a language, per-line patterns |

Every rule takes an occurrence count (`required`, `repeat: { min, max }`) and an `extract`.
A rule can be made conditional on another field's value with `when: { field: X, eq: Y }`.

Validation is **closed-world** by default: a heading or a line the schema never declared is an
error. `open: true` in the schema, or `--open` on the command line, relaxes that for undeclared
structures — but never for a missing requirement or a violated pattern.

## Extraction

`extract` names where a value lands, so the output keys are yours, not the document's. Renaming
a heading in the document does not change the JSON your code reads — you only update the schema.

An item can come back as an object, with the line numbers your own diagnostics need:

```yaml
item:
  id: "REQ-\\d{3,}"
  repeat: { min: 0 }
  extract:
    - requirements
    - { path: id, of: id }
    - { path: line, of: line }
  fields:
    - name: Kind
      extract: kind
    - name: Source
      separator: ","
      extract: sources
  statement:
    extract: text
```

```json
"requirements": [
  { "kind": "ubiquitous", "sources": ["a.md", "b.md"], "text": "...",
    "id": "REQ-005", "line": 8 }
]
```

Extracted values are strings; no type conversion is applied. Line numbers placed with `of: line`
are the one exception, and are numbers.

## Commands and exit codes

```
mds check <path> [--format json|text] [--open]
mds values <file> [--format json|text]
mds ast <file> [--schema] [--format json]
mds --version
```

| Code | Meaning |
|---|---|
| 0 | no findings |
| 1 | findings were reported |
| 2 | the check could not run (schema missing or invalid, broken frontmatter, unreadable file, bad argument) |

## Using it as a library

`mds-core` holds the schema language, validation and extraction, with no CLI dependencies.
Reading files and fetching URLs stays with the caller. The entry points promised to dependent
crates are listed in `docs/ir/library.md`; anything else that happens to be public is an
implementation detail.

```rust
let schema_ref = mds_core::frontmatter::frontmatter_schema(source)?.unwrap();
let location = mds_core::frontmatter::resolve_schema(doc_path, &schema_ref);
// read or fetch `location` yourself, then:
let schema = mds_core::schema::parse_schema(&schema_yaml)?;
let document = mds_core::document::Document::parse(source)?;
let findings = mds_core::validate::validate(&schema, &document, false);
let values = mds_core::extract::extract_values(&schema, &document);
```

## Specification

`docs/spec/mds.md` is the prose specification. `docs/ir/` carries the same specification
normalised so that it can be checked mechanically, and every requirement there is linked to the
tests that cover it.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.
