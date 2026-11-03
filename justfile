# Muster task runner. This file is the single entry point named in CLAUDE.md.
#
# Every recipe below is a gate from the "Gates" table in CLAUDE.md. A recipe that
# prints "not built yet" is a to-do, not a pass: building it is part of the first
# feature that needs it. Do not make a recipe exit 0 until it does what its line in
# CLAUDE.md says.

set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Release build of the binary.
build:
    cargo build --workspace --release

# Unit and integration tests, after the scripts' own tests. The integration tests run
# the built binary against the fake Linear in crates/muster/tests/support/ (ADR 0010).
test *ARGS: test-scripts
    cargo test --workspace {{ARGS}}

# The tests of the Python scripts that have them, standard library only, no network.
test-scripts:
    python3 scripts/release-notes-email-test.py
    python3 scripts/release-notes-test.py

# The README's ten-minute path against a real workspace, with the key from its usual
# sources (ADR 0008). Writes to that workspace: one issue, one comment, one close. Not
# in CI; run it by hand before a release and put its output in the commit message.
smoke:
    bash scripts/smoke.sh

# The README's ten-minute path, timed, end to end, against the fake Linear (CLAUDE.md,
# gate 5): build the binary and the fake, start the fake on a loopback port, run the
# README's commands against it through MUSTER_ENDPOINT, and fail on any step that does
# not do what the README says it does. What CI runs on Linux and macOS.
quickstart:
    bash scripts/quickstart.sh

# clippy with workspace lints, fmt, the workspace lint and the commands lint.
lint: fmt-check clippy lint-workspace lint-commands

# The CLAUDE.md rules rustc and clippy cannot see: the lint inheritance, the dependency
# allowlist, the docs lints on lib.rs and main.rs, and the print lints' one `#[expect]`.
lint-workspace:
    python3 scripts/lint-workspace.py

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# docs/reference/commands.md is generated from the binary (CLAUDE.md, gates 2 to 5;
# ADR 0008): the command tree, the exit codes, the bounds and the environment variables.
gen-commands:
    cargo run --quiet -p muster -- schema --format markdown > docs/reference/commands.md

lint-commands:
    #!/usr/bin/env bash
    set -euo pipefail
    diff -u docs/reference/commands.md <(cargo run --quiet -p muster -- schema --format markdown) \
      || { echo "lint-commands: docs/reference/commands.md is stale; run just gen-commands" >&2; exit 1; }

# Re-record the shapes the fake Linear is held to (ADR 0010): every read operation
# against a real workspace with the key, values stripped to their JSON types, one file
# per operation under crates/muster/tests/fixtures/. Needs the key; not in CI.
record-fixtures:
    cargo run --quiet -p muster -- fixtures crates/muster/tests/fixtures

# rustdoc with warnings denied; the mdBook under target/book with any warning denied;
# the book's Rust code blocks; and every relative link and anchor in docs/ and the
# READMEs (scripts/lint-docs.py links). External links are `just docs-links`, not a
# gate: a site going down is not a bug in this repository (ADR 0006). The mdbook
# version named below is the one CI pins in .github/workflows/ci.yml; bump both.
docs:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v mdbook >/dev/null || { echo "docs: mdbook is not installed. brew install mdbook, or cargo install --locked mdbook@0.5.4" >&2; exit 1; }
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
    out=$(mdbook build 2>&1) || { printf '%s\n' "$out" >&2; exit 1; }
    printf '%s\n' "$out"
    if grep -qE '(^ *|\[)(WARN|ERROR)(\]| )' <<<"$out"; then echo "docs: mdbook warned; warnings are denied" >&2; exit 1; fi
    mdbook test
    python3 scripts/lint-docs.py links

# Fail if a public item or a command changed without docs, a page is missing from
# docs/SUMMARY.md, an ADR is malformed, or an accepted ADR was edited. The range it
# checks is described in the script's header. What CI runs.
lint-docs:
    python3 scripts/lint-docs.py

# External links in docs/ and the READMEs, checked online. Not a gate: run it before a
# release.
docs-links:
    command -v lychee >/dev/null || { echo "docs-links: lychee is not installed. brew install lychee, or cargo install --locked lychee" >&2; exit 1; }
    lychee --no-progress docs README.md crates/*/README.md

# Retire branches whose work is on master, locally and on origin: every branch whose
# pull request merged, or the one named. GitHub deletes a merged pull request's branch
# on origin by itself; the local branch, its worktree and a working branch that never
# became a pull request linger until this runs. Never rebases, force-pushes or aborts a
# merge; a worktree with changes or a merge in progress is named and left alone.
tidy *BRANCH:
    python3 scripts/tidy.py {{BRANCH}}
