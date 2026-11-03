# Cut a release

A how-to page: one task, a release of muster from the version bump to the binaries on
GitHub and the crate on crates.io. Why a release is shaped this way is
[ADR 0011](../decisions/0011-release-as-muster-cli.md). The workflow is
`.github/workflows/release.yml`; its header says what each job does.

## Prerequisites

- Write access to the repository and the house's crates.io token (the maintainers' secrets
  file, under `crates.io`). The token is never put on a command line: it goes in the
  environment variable `CARGO_REGISTRY_TOKEN` for the one `cargo publish` command.
- `Unreleased` in `CHANGELOG.md` holds at least one line a user can act on. A release
  happens when that is true, not on a calendar.
- `master` is green: `gh run list --branch master --workflow ci --limit 1` shows
  `success`.

## Steps

1. **Set the version.** In the root `Cargo.toml`, `[workspace.package] version` becomes
   the new `X.Y.Z`; a change to the command tree or a new command is a minor bump while
   the version is `0.x`, a fix a patch bump. `cargo build` rewrites `Cargo.lock`.
2. **Write the changelog section.** In `CHANGELOG.md`, rename `## Unreleased` to
   `## X.Y.Z - YYYY-MM-DD`, add a fresh `## Unreleased` with `Nothing yet.` above it, and
   open the section with one or two sentences a reader can act on. The section is the
   release's notes, word for word: `python3 scripts/changelog-section.py X.Y.Z` prints
   what the release will say.
3. **Land it.** `/commit` with the subject `Release X.Y.Z`: the four gates run, then the
   commit is pushed to `master`. Wait for CI: `gh run list --branch master --workflow ci
   --limit 1` shows `success`.
4. **Tag it.** On that commit:

   ```sh
   git tag -a vX.Y.Z -m "muster X.Y.Z"
   git push origin vX.Y.Z
   ```

   The `release` workflow starts within a minute: `gh run list --workflow release
   --limit 1`. Its `create` job refuses a tag whose version is not the manifest's, then
   creates the GitHub Release with the changelog section as its notes; its two `build`
   jobs attach `muster-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz` and
   `muster-X.Y.Z-aarch64-apple-darwin.tar.gz`, each with a `.sha256` beside it. About
   four minutes in all.
5. **Publish the crate**, once the release run is green:

   ```sh
   cargo publish -p muster-linear --locked
   ```

   with `CARGO_REGISTRY_TOKEN` set in the environment of that one command, from the
   secrets file, never typed into the shell history. `cargo publish --dry-run` first
   shows what the package holds and catches a manifest problem without a token.

## How to verify it worked

- `gh release view vX.Y.Z` lists the two tarballs and the two sums, with the changelog
  section as the body.
- `curl -sL https://github.com/appshapes/muster/releases/download/vX.Y.Z/muster-X.Y.Z-aarch64-apple-darwin.tar.gz | tar -xz`
  on a Mac, or the Linux name on Linux, gives a `muster` that prints `muster X.Y.Z` for
  `--version`.
- `cargo install muster-linear` on a machine with a toolchain prints
  `Installed package muster-linear vX.Y.Z`, and `muster --version` prints `muster X.Y.Z`.
- <https://crates.io/crates/muster-linear> shows the version and the crate's README.

## When it does not work

- **The `create` job fails with "tag vX.Y.Z does not match the manifest version".** The
  tag was pushed on a commit whose `Cargo.toml` says another version. Delete the tag
  locally and on origin (`git tag -d vX.Y.Z && git push origin :refs/tags/vX.Y.Z`), land
  the version (step 1 to 3), tag again.
- **`cargo publish` answers 403 "this crate exists but you don't seem to be an
  owner".** The name belongs to someone else, which is how `muster-cli` was lost
  ([ADR 0012](../decisions/0012-publish-as-muster-linear.md)). Before an ADR names a
  crate, `curl -s -o /dev/null -w '%{http_code}' https://crates.io/api/v1/crates/<name>`
  prints `404`; a new name is a new ADR and the next patch version, never a moved tag.
- **`cargo publish` answers 403 about permissions.** The token cannot publish this
  crate: it needs the `publish-new` scope for a first release and `publish-update`
  after, on crates.io under Account Settings, API Tokens. Nothing on GitHub is affected;
  publish again with a token that can.
- **`cargo publish` says the version already exists.** crates.io never takes a version
  twice, even after a yank. Bump the patch version and release again.
- **A `build` job fails.** The release exists with the notes and the other tarball; fix
  the build on `master`, delete the tag and the release (`gh release delete vX.Y.Z
  --yes`), and tag the fixed commit with the next patch version.
