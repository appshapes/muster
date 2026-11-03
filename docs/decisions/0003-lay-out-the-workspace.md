# 0003. Lay out the workspace: one crate, one version, no publishing yet, lints in three places

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 2, 5, 6
- **Supersedes:** none

## Context

The first code in the repository is a Cargo workspace with one crate, `muster`, which is
both the library (`muster::Linear`, the client) and the binary. `CLAUDE.md` names gates
that enforce each invariant, and the first of them, `just lint` and `just test`, have to
be real before any command lands, because a gate that prints "not built yet" is a to-do,
not a pass. Cavalry's workspace (Cavalry ADR 0003) settled the same questions for nine
crates; muster has one, and some of Cavalry's machinery is for a shape muster does not
have.

Laying the workspace out forced decisions the brief does not make: whether the crate is
publishable, where each rule lives when rustc, clippy and Cargo each see only part of it,
how a dependency is allowed, and which lints a tool that holds an API key and speaks to
an agent needs that Cavalry does not.

The toolchain is Rust 1.99 on the laptop that made these decisions and the current
stable on CI. Edition 2024 needs 1.85 or later; `ureq` 3.5 needs 1.85.

## Decision

The workspace has `members = ["crates/*"]` and one version, `0.1.0`, in
`[workspace.package]`, which the crate inherits. The crate is `publish = false` until
the first release, which gets its own ADR; that ADR also picks the published name,
because `muster` on crates.io belongs to another project (a tmux session library, at
0.5.6 on 2026-11-02), and the binary stays `muster` whatever the crate is called. The rules live where the tool that can see
them lives. `[workspace.lints.rust]` forbids `unsafe_code`: muster has no reason for
`unsafe` anywhere, so the lint is `forbid`, not `deny`. `[workspace.lints.clippy]`
denies `unwrap_used`, `expect_used`, `panic`, `dbg_macro`, `print_stdout` and
`print_stderr`: a panic is an undocumented exit code (invariant 2), and every byte on
stdout goes through the one module that writes the answer, which carries the only
`#[expect]` of the print lints. The root `clippy.toml` disallows `std::io::stdin`
(muster never prompts), `std::process::exit` (main returns an `ExitCode` so stdout is
flushed and the error object is written) and `std::thread::sleep` (a rate limit is
reported, never absorbed: invariant 3). `scripts/lint-workspace.py`, run by
`just lint-workspace` as part of `just lint`, checks what neither can: every crate
inherits the workspace lints; every direct dependency is on the crate's
`allowed-dependencies` list in `[package.metadata.muster]`; and `src/lib.rs` and
`src/main.rs` carry `#![deny(missing_docs, rustdoc::broken_intra_doc_links)]`. The
Python scripts are standard library only, Python 3.9 or later, as Cavalry's are. The
repository is trunk-based by default; bots and agent workflows use branches and pull
requests (ADR 0004), and Dependabot opens one grouped pull request a week for the
SHA-pinned actions and one for the crates.

## Alternatives considered

- **Two crates, a client library and the binary.** Lost for now: one consumer of the
  client exists, the binary, and a second crate is a second README, a second version
  line and a second set of lints for no reader. Split when a second consumer exists.
- **Publish `0.1.0` now.** Lost: nothing works yet, and a crate whose binary does
  nothing lies to `cargo install`.
- **`#![forbid(unsafe_code)]` in `lib.rs` instead of the Cargo table.** Lost: the Cargo
  table is one place, and the script checks that the crate inherits it.
- **`missing_docs` in the Cargo table too.** Lost: a reader opening `lib.rs` should see
  the lint there, as Cavalry's readers do.
- **Allow `unwrap` and `expect`.** Lost: an `unwrap` on a Linear answer is a panic on
  the first field Linear leaves null, which is an exit code `docs/reference/commands.md`
  does not list. The `?` operator and `ErrorCode` cost nothing more.
- **No dependency allowlist, one crate being small.** Lost: the tool holds an API key,
  and a dependency that reads the environment or the network is the way a key leaves.
  The list is four names and the check is twenty lines.
- **A Rust `xtask` crate instead of Python scripts.** Lost for now, as in Cavalry: it
  compiles on every `just lint`, and Python 3 is on every laptop and both CI runners.
- **Actions pinned by version tag.** Lost: tags are mutable (the tj-actions compromise
  of March 2025, CVE-2025-30066). Every action is pinned to a full commit SHA with its
  version in a trailing comment, and Dependabot moves them.

## Evidence

- Cavalry's lint-in-three-places layout has held since 2026-10-10 over 43 ADRs and nine
  crates (Cavalry ADR 0003 and its CI).
- Verified on this laptop on 2026-11-02 with cargo 1.99.0: a `[workspace.lints.rust]`
  `forbid` is inherited through `[lints] workspace = true`; clippy's
  `disallowed-methods` reads the root `clippy.toml` for the one crate; `cargo metadata`
  exposes `package.metadata`.
- The first `just lint` and `just test` on the empty crate, and the first CI run on both
  Blacksmith labels, are cited in the commit that lands this ADR.

## Consequences

Easier: one version to bump; one place to look for each rule, named here; a dependency
is a visible decision; a panic cannot ship.

Harder: a new dependency needs an allowlist entry, a justification in the commit message
and an ADR, by design; the lint scripts are Python in a Rust repository; a contributor
reaches for `unwrap` and is refused.

Revisit: at the first release, when `publish` flips (new ADR); when a second consumer of
the client exists (split the crate); when a workspace check needs Rust's parser
(`xtask`).

## References

- `CLAUDE.md`: repository map, "Working rules for agents"; ADR 0001, ADR 0002, ADR 0004
- `Cargo.toml`, `clippy.toml`, `scripts/lint-workspace.py`, `.github/workflows/ci.yml`,
  `.github/dependabot.yml`
- Cavalry ADR 0003, `docs/decisions/0003-lay-out-the-workspace.md` in
  <https://github.com/appshapes/cavalry>
