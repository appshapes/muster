# 0002. Write muster in Rust, on a blocking HTTP client, with hand-written operations

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 2, 3, 5, 6
- **Supersedes:** none

## Context

Muster is a command-line client for Linear whose first users are coding agents: Claude
Code sessions working from a checkout, and the writer, reviewer and release-notes agents
that run on CI runners. The owner's brief of 2026-11-02 names Rust as the implementation
language and asks for advice if Rust is not the best option. The brief also says that
muster takes Cavalry's practices: the `justfile` as the single entry point, clippy with
workspace lints, `#![deny(missing_docs)]`, ADRs, the documentation gate, and the six
GitHub Actions workflows that run the agent loop.

Linear exposes one GraphQL endpoint, `https://api.linear.app/graphql`, authenticated
with an API key in the `Authorization` header, an official TypeScript SDK
(`@linear/sdk`) and an MCP server, and no official command-line client. Every
`linear-cli` on GitHub, npm, JSR, crates.io and PyPI is third-party, and the names
collide. A command of muster makes one to three requests and exits; it never holds a
connection open and never waits on two requests at once.

The language is not decisive for a tool that posts JSON and prints JSON. What is
decisive is where the tool runs: on a CI runner with no runtime installed, inside an
agent's `Bash` tool with a short timeout, and on a laptop through `cargo install` or a
release artifact. And what the template costs: Cavalry's gates are Rust-shaped and port
by copying; Brigade's, the other sibling, are Go-shaped.

## Decision

Muster is written in Rust, edition 2024, as one crate that builds one static binary. The
HTTP client is `ureq` 3, blocking, with `rustls` and the bundled roots, so the binary
carries its own TLS and needs no system library. The GraphQL operations are hand-written
strings in `src/linear/operations.rs`, each with a name, deserialized with `serde` into
structs that carry only the fields muster prints. There is no `tokio`, no `reqwest`, and
no schema code generation. The command tree is `clap` with the derive API. The direct
dependencies are `clap`, `serde`, `serde_json` and `ureq`, listed on the allowlist in
`crates/muster/Cargo.toml`, and a fifth needs an ADR.

If Rust were not the choice, the advice is Go: one static binary too, with Brigade's
build machinery to port instead of Cavalry's. TypeScript on `@linear/sdk` is the
official route and the wrong shape for this tool: it needs Node on every runner, an
install step before the first command, and a dependency tree in the hundreds for a tool
that holds an API key.

## Alternatives considered

- **Go.** Lost narrowly: equal on the binary, behind on the template. Cavalry's
  `justfile`, lints, ADR tooling and workflows port by copying; Brigade's `Makefile`,
  `golangci` configuration and `goreleaser` would be a second port. Rust's type system
  also makes the output contract of ADR 0008 (every field is Linear's) a compile-time
  shape rather than a convention.
- **TypeScript on `@linear/sdk`.** Lost: the official SDK is typed and complete, but a
  runner needs Node and an `npm install`, cold start is hundreds of milliseconds, and the
  dependency tree is the largest of the options for the tool that holds the key.
- **Python.** Lost for the same reasons as TypeScript, with no official SDK.
- **Shell around `curl` and `jq`.** Lost: no exit-code discipline, no tests worth the
  name, and quoting GraphQL in shell is a bug factory.
- **`reqwest` with `tokio`.** Lost: a command makes one to three sequential requests and
  exits; an async runtime adds compile time, binary size and a second way to write
  everything for no concurrency muster has. `ureq` 3 with `rustls` is one blocking call.
- **Schema code generation (`graphql-client`, `cynic`).** Lost for now: Linear's schema
  is large, codegen adds build steps and compile time, and the first slice has about a
  dozen operations. Revisited when the operations pass about thirty or a field name
  drifts from the schema unnoticed.
- **A third-party `linear-cli` as a base.** Lost: no official one exists, the names
  collide across registries, and none is built for an agent's exit codes and output.

## Evidence

- Linear's API documentation, read 2026-11-02: the endpoint, the `Authorization: <key>`
  header (no `Bearer`), the rate-limit headers `x-ratelimit-requests-limit`,
  `-remaining` and `-reset`, and the error shapes recorded in this repository's
  `crates/muster/tests/fixtures/` (ADR 0010).
- `ureq` 3.5.1 on crates.io: `default = ["rustls", "gzip"]`, `rust-version = 1.85`.
- Cavalry's gates port with name changes alone: `justfile`, `clippy.toml`,
  `scripts/lint-docs.py`, `scripts/tidy.py`, the six workflows. Brigade's would not.

## Consequences

Easier: one `cargo install` or one release artifact is the whole install; the agent
workflows and the gates are Cavalry's with the names changed; the dependency tree is
four crates deep at the top and auditable.

Harder: every new field or operation is written by hand and held to the recorded shape
by a test, instead of generated; a contributor from the TypeScript SDK finds no SDK here.

Revisit: when the operations pass about thirty (codegen); when a command needs two
requests in flight at once (async); when Linear ships an official CLI (whether muster's
agent contract is still the reason to exist).

## References

- `CLAUDE.md`, invariants 2, 3, 5 and 6; ADR 0003 (the workspace), ADR 0008 (the output
  contract), ADR 0010 (the fake Linear)
- Linear API: <https://developers.linear.app/docs/graphql/working-with-the-graphql-api>
- `ureq`: <https://github.com/algesten/ureq>; `clap`: <https://github.com/clap-rs/clap>
- The owner's brief of 2026-11-02, handed over through Brigade from the Cavalry checkout
