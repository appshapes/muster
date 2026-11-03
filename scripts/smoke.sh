#!/usr/bin/env bash
# The README's ten-minute path against a REAL workspace (CLAUDE.md, gate 1; ADR 0010),
# then the second slice's commands (ADR 0013) and the third's, group by group (ADR
# 0014): the key from its usual sources, the team
# inferred or from MUSTER_TEAM, and one issue created, commented, read, given a
# sub-issue, and then canceled with it (not completed, so that a smoke test never counts
# as work done). Not in CI; run it by hand before a release and put its output in the
# commit message. Prints every JSON line muster printed.
set -euo pipefail
cd "$(dirname "$0")/.."
unset MUSTER_ENDPOINT

cargo build --release -p muster-linear --bin muster
bin=target/release/muster
stamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "smoke: muster whoami"
"$bin" whoami | tee "$tmp/whoami.json"
echo
echo "smoke: muster workspace get"
"$bin" workspace get | jq -c '{name, urlKey, userCount, createdIssueCount, subscription}' | tee "$tmp/workspace.json"
jq -e --slurpfile who "$tmp/whoami.json" '.urlKey == $who[0].organization.urlKey and (.userCount | type) == "number"' "$tmp/workspace.json" >/dev/null
echo
echo "smoke: muster project create --name 'Smoke tests' (exit 6 after the first run: the duplicate guard)"
set +e
"$bin" project create --name "Smoke tests" --description "Where the smoke test's issues go; each is canceled by the run that made it." > "$tmp/project.json" 2> "$tmp/project.err"
code=$?
set -e
case "$code" in
  0) cat "$tmp/project.json" ;;
  6) cat "$tmp/project.err"; jq -e '.error.code == "refused" and (.error.message | test("a project named"))' "$tmp/project.err" >/dev/null ;;
  *) echo "smoke: project create exited $code" >&2; cat "$tmp/project.err" >&2; exit 1 ;;
esac
echo
echo "smoke: muster issue create --project 'Smoke tests'"
"$bin" issue create --title "Smoke test $stamp" --project "Smoke tests" --description "Created by scripts/smoke.sh at $stamp. It is canceled by the same run." | tee "$tmp/create.json"
jq -e '.project.name == "Smoke tests"' "$tmp/create.json" >/dev/null
id=$(jq -r .identifier "$tmp/create.json")
echo
echo "smoke: muster issue comment $id"
"$bin" issue comment "$id" --body "Read back by the smoke test." | tee "$tmp/comment.json"
echo
echo "smoke: muster issue get $id"
"$bin" issue get "$id" | jq '{identifier, title, state: .state.name, comments: [.comments.nodes[].body]}'
echo
echo "smoke: muster issue create --parent $id"
"$bin" issue create --title "Smoke test sub-issue $stamp" --parent "$id" --description "A sub-issue of $id, canceled by the same run." | tee "$tmp/child.json"
child=$(jq -r .identifier "$tmp/child.json")
jq -e '.parent.identifier == "'"$id"'"' "$tmp/child.json" >/dev/null
echo
echo "smoke: muster issue get $id (children)"
"$bin" issue get "$id" | jq -c '{identifier, children: [.children.nodes[] | {identifier, state: .state.name}]}' | tee "$tmp/children.json"
jq -e '.children[0].identifier == "'"$child"'"' "$tmp/children.json" >/dev/null
echo
echo "smoke: muster issue attach $id"
"$bin" issue attach "$id" --url "https://github.com/appshapes/muster/blob/master/scripts/smoke.sh" --title "The smoke script" --subtitle "attached at $stamp" | tee "$tmp/attach.json"
jq -e '.issue.identifier == "'"$id"'" and .title == "The smoke script"' "$tmp/attach.json" >/dev/null
echo
echo "smoke: muster issue attach $id, the same URL again"
"$bin" issue attach "$id" --url "https://github.com/appshapes/muster/blob/master/scripts/smoke.sh" --title "The smoke script, renamed" | jq -c '{id, title, subtitle}' | tee "$tmp/attach2.json"
jq -e --slurpfile first "$tmp/attach.json" '.id == $first[0].id and .title == "The smoke script, renamed" and .subtitle == $first[0].subtitle' "$tmp/attach2.json" >/dev/null
echo
echo "smoke: muster issue get $id (attachments)"
"$bin" issue get "$id" | jq -c '{identifier, attachments: [.attachments.nodes[] | {title, url}]}' | tee "$tmp/attachments.json"
jq -e '(.attachments | length) == 1 and .attachments[0].title == "The smoke script, renamed"' "$tmp/attachments.json" >/dev/null
echo
echo "smoke: muster issue list --project 'Smoke tests'"
"$bin" issue list --project "Smoke tests" | jq -c '{identifier, project: .project.name}' | tee "$tmp/in-project.jsonl"
jq -s -e --arg id "$id" 'map(.identifier) | index($id) != null' "$tmp/in-project.jsonl" >/dev/null
echo
echo "smoke: muster issue list --parent $id"
"$bin" issue list --parent "$id" | jq -c '{identifier, parent: .parent.identifier}'
echo
echo "smoke: muster issue update $child --parent none"
"$bin" issue update "$child" --parent none | jq -c '{identifier, parent}' | tee "$tmp/orphan.json"
jq -e '.parent == null' "$tmp/orphan.json" >/dev/null
echo
echo "smoke: muster issue close $child --cancel"
"$bin" issue close "$child" --cancel --comment "Smoke test sub-issue done at $stamp; canceled so that it never counts as work." | jq -c 'if .identifier then {identifier, state: .state.name} else {comment: .body} end'
echo
echo "smoke: muster issue close $id --cancel"
"$bin" issue close "$id" --cancel --comment "Smoke test done at $stamp; canceled so that it never counts as work." | tee "$tmp/close.jsonl"
jq -s -e '.[1].state.type == "canceled"' "$tmp/close.jsonl" >/dev/null
echo
echo "smoke: ok ($id and $child canceled)"
