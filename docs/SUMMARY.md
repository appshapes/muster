# Summary

The four kinds of page, in the order `.claude/rules/docs.md` defines them. Every page
under `docs/` is listed here; `just lint-docs` fails on one that is not.

# Tutorials

- [Work an issue in ten minutes](tutorials/work-an-issue-in-ten-minutes.md)

# Explanation

- [Why Muster, and why a CLI for agents](explanation/why-muster.md)

# How-to

- [Ask the agent to make a change](how-to/ask-the-agent.md)
- [Implement a feature, from work item to commit](how-to/implement-a-feature.md)
- [Send the weekly release-notes email](how-to/send-the-release-notes.md)
- [Cut a release](how-to/cut-a-release.md)

# Reference

- [Commands, exit codes, bounds and environment](reference/commands.md)
- [Glossary](glossary.md)

# Decisions

- [ADR template](decisions/0000-template.md)
- [0001. Record decisions as ADRs in the repository](decisions/0001-record-decisions-as-adrs.md)
- [0002. Write muster in Rust, on a blocking HTTP client, with hand-written operations](decisions/0002-write-muster-in-rust-on-a-blocking-client.md)
- [0003. Lay out the workspace: one crate, one version, no publishing yet, lints in three places](decisions/0003-lay-out-the-workspace.md)
- [0004. Run agents on branches, with the owner's token out of every agent's job](decisions/0004-run-agents-on-branches.md)
- [0005. Review pull requests in the base repository's context](decisions/0005-review-in-the-base-context.md)
- [0006. Build the documentation gate from mdBook, rustdoc and a link script](decisions/0006-build-the-docs-gate.md)
- [0007. Build the Trello command set first, in five features](decisions/0007-build-the-trello-command-set-first.md)
- [0008. Speak to agents first: JSON Lines, exit codes, no prompt, the key from a file](decisions/0008-speak-to-agents-first.md)
- [0009. Email the week's changes to a list, drafted by an agent and gated before it is sent](decisions/0009-email-the-weeks-changes.md)
- [0010. Test against a fake Linear in process, held to recorded response shapes](decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md)
- [0011. Release as `muster-cli` on crates.io and as binaries on GitHub, from a tag](decisions/0011-release-as-muster-cli.md)
- [0012. Publish as `muster-linear`: `muster-cli` is taken as well](decisions/0012-publish-as-muster-linear.md)
- [0013. Build the second slice: sub-issues, attachments and projects, in three features](decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md)
- [0014. Build the third slice: administrative commands, in twenty groups](decisions/0014-build-the-third-slice-administrative-commands.md)
- [0015. Carry Linear's `userPresentableMessage` on the error object, beside its message](decisions/0015-carry-linears-user-presentable-message-on-the-error-object.md)
- [0016. Track muster's own work on the Brigade Board, not in Linear](decisions/0016-track-musters-own-work-on-the-brigade-board.md)
