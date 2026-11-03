# 0012. Publish as `muster-linear`: `muster-cli` is taken as well

- **Status:** Accepted
- **Date:** 2026-11-03
- **Invariants affected:** 5, 6
- **Supersedes:** the package name of ADR 0011; the rest of ADR 0011 stands

## Context

ADR 0011 named the package `muster-cli` because `muster` on crates.io belongs to another
project. The check behind that decision looked up `muster` alone. On 2026-11-03, with
`v0.1.0` tagged and its GitHub Release built, `cargo publish -p muster-cli` answered
`403 Forbidden: this crate exists but you don't seem to be an owner`: `muster-cli` 0.5.6,
"Terminal session group management CLI", belongs to the same project as `muster`, which
also holds `muster-workspace`. Release 0.1.0 therefore exists on GitHub, with its two
tarballs and their sums, and nowhere on crates.io, while its notes, its tarballs' README
and the repository's pages say `cargo install muster-cli`, which installs a stranger's
tool.

A crates.io version is never taken twice, a tag is not moved, and an accepted ADR is not
edited beyond its Status (ADR 0001). So the fix is a new name, a new ADR, the next patch
version, and a correction on the notes of 0.1.0.

## Decision

The package is `muster-linear`. The binary and the library stay `muster`, as ADR 0011
has them, and everything else in that ADR stands: the tag, the release workflow, the two
tarballs, the manual publish. The name says what the tool is for and puts the word a
user searches for first. Version 0.1.1 is the first release on crates.io; 0.1.0 stays on
GitHub as it is, with a correction at the top of its notes saying that the crate name in
them is wrong and where to install from. From now on, before an ADR names a crate,
`curl -s -o /dev/null -w '%{http_code}' https://crates.io/api/v1/crates/<name>` prints
`404`, and the ADR cites the check; `docs/how-to/cut-a-release.md` says so under "When it
does not work".

## Alternatives considered

- **`linear-muster`.** Lost: the word a user types and searches for is `muster`; it
  goes first.
- **`appshapes-muster`.** Lost: a house prefix means nothing to a user outside the
  house, and the tool is not house-specific.
- **`musterctl`.** Lost: the `ctl` suffix says "controls a daemon"; muster controls
  nothing, it speaks to Linear.
- **`muster-agent`, `muster-issues`.** Lost: muster is for agents but is not one, and
  issues are the first slice, not the whole surface.
- **Asking the other project for `muster-cli`.** Lost: a conversation with a stranger
  with no deadline, while the tool has users waiting on `cargo install` today; nothing
  stops it being asked later.
- **Re-tagging 0.1.0 after the rename.** Lost: a tag that moved and a release whose
  tarballs changed under the same name are two things a reader cannot trust again;
  0.1.1 costs one number.

## Evidence

- `cargo publish -p muster-cli --locked` on 2026-11-03: packaged 30 files, verified,
  then `403 Forbidden` as quoted above.
- crates.io API on 2026-11-03: `muster-cli` at 0.5.6, created 2026-03-18, and
  `muster-workspace` at 0.10.3, both of the project that holds `muster`;
  `muster-linear`, `linear-muster`, `appshapes-muster`, `musterctl`, `muster-agent` and
  `muster-issues` all `404`.
- The 0.1.1 release run and the crates.io publish are cited in the commit that tags
  `v0.1.1`.

## Consequences

Easier: `cargo install muster-linear` is a crate of this repository; a search for
`muster` on crates.io lists it beside the other project's, with a description that tells
them apart.

Harder: the first two days of the repository name three different crates; the 0.1.0
release on GitHub carries a correction; a reader of ADR 0011 must read this one.

Revisit: if the other project ever offers `muster` or `muster-cli`; the binary and the
library would keep their names either way.

## References

- ADR 0011, ADR 0001; `docs/how-to/cut-a-release.md`, `CHANGELOG.md`
- crates.io API: <https://crates.io/api/v1/crates/muster-cli>,
  <https://crates.io/api/v1/crates/muster-linear>
- The GitHub Release `v0.1.0` and its correction
