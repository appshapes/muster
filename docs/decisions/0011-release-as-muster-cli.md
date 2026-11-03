# 0011. Release as `muster-cli` on crates.io and as binaries on GitHub, from a tag

- **Status:** Superseded by 0012
- **Date:** 2026-11-03
- **Invariants affected:** 5, 6
- **Supersedes:** the `publish = false` clause of ADR 0003; the rest of ADR 0003 stands

## Context

The first slice of ADR 0007 is on `master`: the five Trello operations as `muster`
commands, 25 tests through the binary against the fake Linear, `just quickstart` green
on both CI runners, `just smoke` run against the appshapes workspace (APP-5 created,
commented, read back and canceled), and the agent loop proven on Dependabot's pull
request 1, reviewed and squash-merged with no person in the path. The owner asked for a
release on 2026-11-03 and, earlier, whether to reserve the crate name.

The name `muster` on crates.io is taken: another project, a tmux session library, has
held it since before this repository existed and published 0.5.6 on 2026-05-05. A crate
name cannot be reserved by a second party, and asking for a transfer is a conversation
with a stranger that may never conclude. ADR 0003 left the published name to the first
release's ADR for this reason.

A release has two audiences: a person or a CI job that has a Rust toolchain, for whom
`cargo install` is the whole install; and an agent's runner or a laptop without one, for
whom a prebuilt binary is. Muster is one static binary with `rustls` and bundled roots
(ADR 0002), so a tarball per platform needs no system library.

## Decision

The package is `muster-cli`, and the binary and the library it builds are both `muster`
(`[[bin]] name = "muster"`, `[lib] name = "muster"`), the pattern `sqlx-cli` and
`diesel_cli` follow: `cargo install muster-cli` puts `muster` on the path, and
`cargo add muster-cli` gives `muster::Linear`. The directory stays `crates/muster`, the
`justfile` names the package where cargo needs it, and every other name in the
repository is `muster`.

The version is the workspace's, `0.1.0` first, and a release is three steps a person
takes, written in `docs/how-to/cut-a-release.md`: the version and the `CHANGELOG.md`
section land on `master` through the gates like any commit; a tag `vX.Y.Z` on that
commit starts `.github/workflows/release.yml`, which refuses a tag whose version is not
the manifest's, creates the GitHub Release with the changelog section as its notes,
builds `muster` on the two CI runners with `--locked`, and attaches
`muster-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz` and
`muster-X.Y.Z-aarch64-apple-darwin.tar.gz`, each holding the binary, `README.md` and
`LICENSE`, with a SHA-256 file beside each; and `cargo publish -p muster-cli` runs from a
maintainer's laptop with the house's crates.io token, once CI is green on the tagged
commit. The crates.io package is the source and the `README.md` of the crate; the GitHub
Release is the binaries. A release happens when `Unreleased` in `CHANGELOG.md` holds a
line a user can act on, not on a calendar.

## Alternatives considered

- **`muster` as the package name.** Lost: taken by another project since before this
  one existed; crates.io does not reserve names for a second party.
- **`appshapes-muster` or `linear-muster`.** Lost: a prefix no user types looks for;
  `-cli` is the suffix the ecosystem already reads as "the binary of this name".
- **A binary-only crate, no library.** Lost: the tests and the README's example use
  the library, and the README is the crate's rustdoc front page (CLAUDE.md,
  invariant 6); a library costs nothing to ship.
- **Publishing to crates.io from the release workflow with a token secret.** Lost for
  now: a publish token in the repository's secrets is reachable by every workflow that
  asks for it, the agent workflows included, and the publish is one command a person
  runs once per release. Revisited when releases are frequent enough to be a burden.
- **Static musl binaries for Linux.** Lost for now: the gnu target runs on every
  current distribution, and the runner image builds it with no cross toolchain. A musl
  target joins the matrix when a user needs it.
- **A Homebrew tap or a `cargo-binstall` manifest.** Lost for now: the tarball names
  follow `cargo-binstall`'s default convention, so a manifest can be added without a
  new ADR; a tap is a second repository to maintain.
- **Semantic version `1.0.0` for the first release.** Lost: the output shape is an
  append-only contract (ADR 0008) but the command tree will grow through the next
  slices; `0.x` says so.

## Evidence

- crates.io on 2026-11-02: `muster` is at 0.5.6 by another author, description
  "Terminal session group management library built on tmux".
- `master` at the time of this decision: CI runs 41559448574 and 41559548234 green on
  both runners; the loop on pull request 1 (dispatch run 41559218217, reviewer run
  41559232849, auto-merge run 41559523746, squash commit 56d75c0); `just smoke` on
  2026-11-03 against the appshapes workspace.
- The first release's run ids and the crates.io publish are cited in the commit that
  tags `v0.1.0`.

## Consequences

Easier: `cargo install muster-cli` on any machine with a toolchain, one tarball
elsewhere; a release is a tag; the changelog section is the release notes, so the
notes are written in the same commit as the change.

Harder: the package name and the binary name differ, which the README, the tutorial
and the crate's own README say; the crates.io publish is a manual step with a token;
two platforms are built, and a third is a matrix row and a new tarball name.

Revisit: when a musl or a Windows build is asked for; when releases are frequent
enough that the manual publish is a cost; if the owner of `muster` on crates.io ever
offers the name.

## References

- ADR 0002, ADR 0003, ADR 0007, ADR 0008
- `crates/muster/Cargo.toml`, `.github/workflows/release.yml`,
  `scripts/changelog-section.py`, `docs/how-to/cut-a-release.md`, `CHANGELOG.md`
- crates.io API: <https://crates.io/api/v1/crates/muster>; `cargo-binstall` naming:
  <https://github.com/cargo-bins/cargo-binstall>
