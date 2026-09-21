# Agent Instructions

## Core

- Serve the stated goal; do not widen the requested scope.
- Distinguish what is confirmed from what is inferred and what is unverified.
- After changing something, verify it by a means appropriate to the change.
- Do not perform irreversible, destructive, or externally visible actions without approval.
- Apply the project's own instructions where they are more specific than these.

## Rule Routing

| When | Read |
|---|---|
| Always | ba0918-design, ba0918-placement, ba0918-readability, ba0918-secrets |
| design | ba0918-reuse |
| implement | ba0918-tdd |
| review | ba0918-verification |
| commit | ba0918-commit |
| release | ba0918-release |
| delegate | ba0918-delegation |
| diff-review | ba0918-diff-review |

Refer to each rule by its skill name. Read every rule that applies before starting the work it
governs.

## Project Context

Project-specific context — what this repository is, how to build and test it, and the
conventions that apply only here — lives in `PROJECT.md`. Read it before making changes.
