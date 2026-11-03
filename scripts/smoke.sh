#!/usr/bin/env bash
# The README's ten-minute path against a REAL workspace (CLAUDE.md, gate 1; ADR 0010),
# then the second slice's commands (ADR 0013) and the third's, group by group (ADR
# 0014): the key from its usual sources, the team from MUSTER_TEAM, else the first team
# that is not the smoke team `SMK` (which this script creates, so the workspace has
# several teams and none is inferred), and one issue created, commented, read, given a
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
# The smoke team (ADR 0014) makes this a workspace with several teams, so the issue
# steps name the one they always used: MUSTER_TEAM when set, else the first team that
# is not SMK.
export MUSTER_TEAM="${MUSTER_TEAM:-$("$bin" team list | jq -r 'select(.key != "SMK") | .key' | head -1)}"
echo "smoke: the issue steps use team $MUSTER_TEAM (MUSTER_TEAM)"
echo
echo "smoke: muster workspace get"
"$bin" workspace get | jq -c '{name, urlKey, userCount, createdIssueCount, subscription}' | tee "$tmp/workspace.json"
jq -e --slurpfile who "$tmp/whoami.json" '.urlKey == $who[0].organization.urlKey and (.userCount | type) == "number"' "$tmp/workspace.json" >/dev/null
echo
echo "smoke: muster workspace update --project-update-reminders-hour <the hour it has> (a change to the same value: the path is proven and nothing moves)"
hour=$("$bin" workspace get | jq -r .projectUpdateRemindersHour)
"$bin" workspace update --project-update-reminders-hour "$hour" | jq -c '{urlKey, projectUpdateRemindersHour, updatedAt}' | tee "$tmp/workspace-update.json"
jq -e --argjson hour "$hour" '.projectUpdateRemindersHour == $hour' "$tmp/workspace-update.json" >/dev/null
echo
echo "smoke: muster user get me, then user update me with a status that clears at once (ADR 0014)"
"$bin" user get me | jq -c '{displayName, admin, owner, guest, active, timezone}' | tee "$tmp/user.json"
jq -e --slurpfile who "$tmp/whoami.json" '.displayName == $who[0].viewer.displayName and .active == true' "$tmp/user.json" >/dev/null
"$bin" user update me --status-emoji "🧪" --status-label "Smoke test at $stamp" --status-until "$stamp" | jq -c '{displayName, statusEmoji, statusLabel, statusUntilAt}' | tee "$tmp/user-update.json"
jq -e --arg stamp "Smoke test at $stamp" '.statusLabel == $stamp' "$tmp/user-update.json" >/dev/null
echo
echo "smoke: muster team create --name 'Smoke tests' --key SMK (exit 6 after the first run: the duplicate guard; ADR 0014)"
set +e
"$bin" team create --name "Smoke tests" --key SMK --description "The smoke test's own team (ADR 0014): where its cycles, members and lifecycle are tested, so that APP is never touched." --timezone Etc/UTC > "$tmp/team.json" 2> "$tmp/team.err"
code=$?
set -e
case "$code" in
  0) jq -c '{key, name, cyclesEnabled, timezone}' "$tmp/team.json" ;;
  6) cat "$tmp/team.err"; jq -e '.error.code == "refused" and (.error.message | test("a team named"))' "$tmp/team.err" >/dev/null ;;
  *) echo "smoke: team create exited $code" >&2; cat "$tmp/team.err" >&2; exit 1 ;;
esac
echo
echo "smoke: muster team get SMK"
"$bin" team get SMK | jq -c '{key, name, cyclesEnabled, retiredAt, archivedAt, issueCount}' | tee "$tmp/team-get.json"
jq -e '.key == "SMK" and .archivedAt == null' "$tmp/team-get.json" >/dev/null
echo
# The start date is set once: setting it again on a team whose cycles run would shift
# its cycles on every run (seen on 2026-11-03, one new cycle per run).
if [ "$(jq -r '.cyclesEnabled' "$tmp/team-get.json")" = "true" ]; then cycle_start=""; else cycle_start="--cycle-enabled-start-date $stamp"; fi
echo "smoke: muster team update SMK --cycles-enabled true ${cycle_start:-(the start date was set by an earlier run)} (the smoke team's cycles; ADR 0014)"
"$bin" team update SMK --cycles-enabled true $cycle_start --cycle-duration 1 --upcoming-cycle-count 2 --description "The smoke test's own team (ADR 0014): its cycles, members and lifecycle are tested here, so that the house's team is never touched." | jq -c '{key, cyclesEnabled, cycleDuration, upcomingCycleCount, activeCycle}' | tee "$tmp/team-update.json"
jq -e '.key == "SMK" and .cyclesEnabled == true and .cycleDuration == 1' "$tmp/team-update.json" >/dev/null
echo
echo "smoke: muster cycle list --team SMK, --active, get, update --name, then an SMK issue through --cycle active, next and none (ADR 0014)"
"$bin" cycle list --team SMK | jq -c '{number, name, isActive, isNext, isPast}' | tee "$tmp/cycles.jsonl"
jq -s -e 'length >= 1' "$tmp/cycles.jsonl" >/dev/null
"$bin" cycle list --team SMK --active | jq -c '{number, isActive}' | tee "$tmp/cycle-active.json"
jq -e '.isActive == true' "$tmp/cycle-active.json" >/dev/null
active_cycle=$(jq -r '.number' "$tmp/cycle-active.json")
"$bin" cycle get "$active_cycle" --team SMK | jq -c '{number, issues: (.issues.nodes | length)}' | tee "$tmp/cycle-get.json"
jq -e --argjson n "$active_cycle" '.number == $n' "$tmp/cycle-get.json" >/dev/null
"$bin" cycle update "$active_cycle" --team SMK --name "Smoke cycle" --description "Named by scripts/smoke.sh at $stamp." | jq -c '{number, name}' | tee "$tmp/cycle-update.json"
jq -e '.name == "Smoke cycle"' "$tmp/cycle-update.json" >/dev/null
"$bin" issue create --team SMK --title "Smoke cycle issue $stamp" --cycle active --description "Created by scripts/smoke.sh at $stamp in the smoke team's active cycle; canceled by the same run." | jq -c '{identifier, cycle: .cycle.number}' | tee "$tmp/cycle-issue.json"
jq -e --argjson n "$active_cycle" '.cycle == $n' "$tmp/cycle-issue.json" >/dev/null
cycle_issue=$(jq -r '.identifier' "$tmp/cycle-issue.json")
"$bin" issue list --team SMK --cycle active | jq -c '{identifier, cycle: .cycle.number}' | tee "$tmp/cycle-issues.jsonl"
jq -s -e --arg id "$cycle_issue" 'map(.identifier) | index($id) != null' "$tmp/cycle-issues.jsonl" >/dev/null
"$bin" issue update "$cycle_issue" --cycle next | jq -c '{identifier, cycle: .cycle.number}' | tee "$tmp/cycle-next.json"
jq -e --argjson n "$active_cycle" '.cycle > $n' "$tmp/cycle-next.json" >/dev/null
"$bin" issue update "$cycle_issue" --cycle none | jq -c '{identifier, cycle}' | tee "$tmp/cycle-none.json"
jq -e '.cycle == null' "$tmp/cycle-none.json" >/dev/null
# Back into the active cycle, then canceled, so that the cycle fixture records an issue node.
"$bin" issue update "$cycle_issue" --cycle active | jq -e --argjson n "$active_cycle" '.cycle.number == $n' >/dev/null
"$bin" issue close "$cycle_issue" --cancel --comment "Smoke cycle issue done at $stamp; canceled so that it never counts as work." | jq -c 'select(.identifier != null) | {identifier, state: .state.name}'
echo
echo "smoke: muster member list --team SMK, then remove me and add me back (ADR 0014)"
"$bin" --team SMK member list | jq -c '{user: .user.displayName, owner}' | tee "$tmp/members.jsonl"
jq -s -e 'length >= 1' "$tmp/members.jsonl" >/dev/null
"$bin" --team SMK member remove --user me | tee "$tmp/member-remove.json"
jq -e '.success == true' "$tmp/member-remove.json" >/dev/null
"$bin" --team SMK member add --user me | jq -c '{user: .user.displayName, team: .team.key, owner}' | tee "$tmp/member-add.json"
jq -e '.team == "SMK"' "$tmp/member-add.json" >/dev/null
echo
echo "smoke: muster team retire SMK, then unretire"
"$bin" team retire SMK | jq -c '{key, retiredAt}' | tee "$tmp/retire.json"
jq -e '.retiredAt != null' "$tmp/retire.json" >/dev/null
"$bin" team unretire SMK | jq -c '{key, retiredAt}' | tee "$tmp/unretire.json"
jq -e '.retiredAt == null' "$tmp/unretire.json" >/dev/null
echo
echo "smoke: muster team delete SMK, then unarchive (the only team the lifecycle is tested on; ADR 0014)"
"$bin" team delete SMK | tee "$tmp/delete.json"
jq -e '.success == true and (.entityId | type) == "string"' "$tmp/delete.json" >/dev/null
"$bin" team unarchive SMK | jq -c '{key, archivedAt}' | tee "$tmp/unarchive.json"
jq -e '.key == "SMK" and .archivedAt == null' "$tmp/unarchive.json" >/dev/null
"$bin" team list | jq -c 'select(.key == "SMK") | {key, retiredAt}' | tee "$tmp/team-list.json"
jq -e '.key == "SMK"' "$tmp/team-list.json" >/dev/null
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
# In SMK, not in $MUSTER_TEAM: nothing unarchives a state, so each run leaves one
# archived state in the team it ran in, for good; the smoke team is where that is kept.
echo "smoke: muster --team SMK state create --name 'Smoke <stamp>' --type backlog, update, archive, in the smoke team, which keeps the archived state (ADR 0014)"
"$bin" --team SMK state create --name "Smoke $stamp" --type backlog --color "#bec2c8" --description "Made by scripts/smoke.sh at $stamp and archived by the same run." | jq -c '{name, type, position, team: .team.key}' | tee "$tmp/state.json"
jq -e --arg n "Smoke $stamp" '.name == $n and .type == "backlog" and .team == "SMK"' "$tmp/state.json" >/dev/null
"$bin" --team SMK state update "Smoke $stamp" --name "Smoky $stamp" | jq -c '{name, type}' | tee "$tmp/state-update.json"
jq -e --arg n "Smoky $stamp" '.name == $n' "$tmp/state-update.json" >/dev/null
"$bin" --team SMK state archive "Smoky $stamp" | jq -c '{name, type}' | tee "$tmp/state-archive.json"
jq -e --arg n "Smoky $stamp" '.name == $n' "$tmp/state-archive.json" >/dev/null
"$bin" --team SMK state list | jq -c 'select(.name == "Smoky '"$stamp"'")' | tee "$tmp/state-gone.jsonl"
[ ! -s "$tmp/state-gone.jsonl" ] || { echo "smoke: the archived state is still listed" >&2; exit 1; }
echo
echo "smoke: muster label create --name 'Smoke <stamp>', update, retire, restore, delete, in team $MUSTER_TEAM (ADR 0014)"
"$bin" label create --name "Smoke $stamp" --color "#26b5ce" --description "Made by scripts/smoke.sh at $stamp and deleted by the same run." | jq -c '{name, color, team: .team.key, retiredAt}' | tee "$tmp/label.json"
jq -e --arg n "Smoke $stamp" '.name == $n and .retiredAt == null' "$tmp/label.json" >/dev/null
"$bin" label update "Smoke $stamp" --name "Smoky $stamp" | jq -c '{name}' | tee "$tmp/label-update.json"
jq -e --arg n "Smoky $stamp" '.name == $n' "$tmp/label-update.json" >/dev/null
"$bin" label retire "Smoky $stamp" | jq -c '{name, retiredAt}' | tee "$tmp/label-retire.json"
jq -e '.retiredAt != null' "$tmp/label-retire.json" >/dev/null
"$bin" label restore "Smoky $stamp" | jq -c '{name, retiredAt}' | tee "$tmp/label-restore.json"
jq -e '.retiredAt == null' "$tmp/label-restore.json" >/dev/null
"$bin" label delete "Smoky $stamp" | tee "$tmp/label-delete.json"
jq -e '.success == true' "$tmp/label-delete.json" >/dev/null
echo
echo "smoke: muster project-status list, then a status created, updated, archived and unarchived (ADR 0014)"
"$bin" project-status list | jq -c '{name, type, position}' | tee "$tmp/statuses.jsonl"
jq -s -e 'length >= 1' "$tmp/statuses.jsonl" >/dev/null
# A project status's name is at most 25 characters (Linear's validation, read on
# 2026-11-03), so the status is named after the run's time of day.
short="${stamp:11:8}"
"$bin" project-status create --name "Smoke $short" --type planned --color "#f2c94c" --position 99 --description "Made by scripts/smoke.sh at $stamp and archived by the same run; a second planned status, since Linear keeps the last of a type." | jq -c '{name, type, position, indefinite}' | tee "$tmp/status-create.json"
jq -e --arg n "Smoke $short" '.name == $n and .type == "planned"' "$tmp/status-create.json" >/dev/null
"$bin" project-status update "Smoke $short" --name "Smoky $short" --indefinite true | jq -c '{name, indefinite}' | tee "$tmp/status-update.json"
jq -e --arg n "Smoky $short" '.name == $n' "$tmp/status-update.json" >/dev/null
"$bin" project-status archive "Smoky $short" | jq -c '{name, type}' | tee "$tmp/status-archive.json"
jq -e --arg n "Smoky $short" '.name == $n' "$tmp/status-archive.json" >/dev/null
"$bin" project-status unarchive "Smoky $short" | jq -c '{name}' | tee "$tmp/status-unarchive.json"
jq -e --arg n "Smoky $short" '.name == $n' "$tmp/status-unarchive.json" >/dev/null
"$bin" project-status archive "Smoky $short" | jq -c '{name}' >/dev/null
echo
echo "smoke: muster initiative create --name 'Smoke initiative' (exit 6 after the first run; it stays, for the fixture), add-project, get, remove-project, then a throwaway initiative updated, archived and unarchived (ADR 0014)"
if [ "$("$bin" workspace get | jq -r '.roadmapEnabled')" != "true" ]; then
  echo "smoke: initiatives are off in this workspace; muster workspace update --roadmap true switches them on, and they stay on"
  "$bin" workspace update --roadmap true | jq -c '{name, roadmapEnabled}'
fi
set +e
"$bin" initiative create --name "Smoke initiative" --description "The initiative the smoke project goes in; it stays so that the fixture has a node (ADR 0014)." --owner me --status Active > "$tmp/initiative.json" 2> "$tmp/initiative.err"
code=$?
set -e
case "$code" in
  0) jq -c '{name, status, owner: .owner.displayName}' "$tmp/initiative.json" ;;
  6) cat "$tmp/initiative.err"; jq -e '.error.code == "refused"' "$tmp/initiative.err" >/dev/null ;;
  *) echo "smoke: initiative create exited $code" >&2; cat "$tmp/initiative.err" >&2; exit 1 ;;
esac
if jq -e '.error.linear.code == "FORBIDDEN" or .error.linear.code == "FEATURE_NOT_ACCESSIBLE"' "$tmp/initiative.err" >/dev/null 2>&1; then
  echo "smoke: the plan does not allow initiatives through the API; the initiative steps are skipped"
else
  "$bin" initiative add-project "Smoke initiative" --project "Smoke tests" > "$tmp/initiative-link.json" 2> "$tmp/initiative-link.err" || { cat "$tmp/initiative-link.err"; jq -e '.error.code == "refused"' "$tmp/initiative-link.err" >/dev/null; }
  "$bin" initiative get "Smoke initiative" | jq -c '{name, status, projects: [.projects.nodes[].name]}' | tee "$tmp/initiative-get.json"
  jq -e '.projects | index("Smoke tests") != null' "$tmp/initiative-get.json" >/dev/null
  "$bin" initiative remove-project "Smoke initiative" --project "Smoke tests" | jq -c '{success}' | tee "$tmp/initiative-unlink.json"
  jq -e '.success == true' "$tmp/initiative-unlink.json" >/dev/null
  "$bin" initiative get "Smoke initiative" | jq -e '.projects.nodes | length == 0' >/dev/null
  "$bin" initiative add-project "Smoke initiative" --project "Smoke tests" | jq -c '{project: .project.name, initiative: .initiative.name}' | tee "$tmp/initiative-relink.json"
  jq -e '.project == "Smoke tests"' "$tmp/initiative-relink.json" >/dev/null
  "$bin" initiative list --status Active | jq -c '{name, status}' | tee "$tmp/initiatives.jsonl"
  jq -s -e 'map(.name) | index("Smoke initiative") != null' "$tmp/initiatives.jsonl" >/dev/null
  "$bin" initiative create --name "Smoke $stamp" --status Planned --priority 4 | jq -c '{name, status, priority}' | tee "$tmp/initiative-throwaway.json"
  jq -e --arg n "Smoke $stamp" '.name == $n' "$tmp/initiative-throwaway.json" >/dev/null
  "$bin" initiative update "Smoke $stamp" --name "Smoky $stamp" --status Canceled --target-date none | jq -c '{name, status}' | tee "$tmp/initiative-update.json"
  jq -e --arg n "Smoky $stamp" '.name == $n and .status == "Canceled"' "$tmp/initiative-update.json" >/dev/null
  "$bin" initiative archive "Smoky $stamp" | jq -c '{name, archivedAt}' | tee "$tmp/initiative-archive.json"
  jq -e '.archivedAt != null' "$tmp/initiative-archive.json" >/dev/null
  "$bin" initiative unarchive "Smoky $stamp" | jq -c '{name, archivedAt}' | tee "$tmp/initiative-unarchive.json"
  jq -e '.archivedAt == null' "$tmp/initiative-unarchive.json" >/dev/null
  "$bin" initiative archive "Smoky $stamp" | jq -c '{name}' >/dev/null
  echo "smoke: muster initiative update --lead-team SMK (a team initiative; exit 6 with FEATURE_NOT_ACCESSIBLE below the Business plan)"
  set +e
  "$bin" initiative update "Smoke initiative" --lead-team SMK > "$tmp/initiative-lead.json" 2> "$tmp/initiative-lead.err"
  code=$?
  set -e
  case "$code" in
    0) jq -c '{name, leadTeam: .leadTeam.key}' "$tmp/initiative-lead.json" ;;
    6) cat "$tmp/initiative-lead.err"; jq -e '.error.linear.code == "FEATURE_NOT_ACCESSIBLE"' "$tmp/initiative-lead.err" >/dev/null; echo "smoke: the plan does not sell team initiatives; --lead-team is refused as documented" ;;
    *) echo "smoke: initiative update --lead-team exited $code" >&2; cat "$tmp/initiative-lead.err" >&2; exit 1 ;;
  esac
fi
echo
echo "smoke: muster document create --title 'Smoke document' --project 'Smoke tests' (exit 6 after the first run; it stays, for the fixture), get, list, then a throwaway document updated, deleted, unarchived and deleted again (ADR 0014)"
set +e
"$bin" document create --title "Smoke document" --project "Smoke tests" --content "The smoke project's document; it stays so that the fixture has a node (ADR 0014)." > "$tmp/document.json" 2> "$tmp/document.err"
code=$?
set -e
case "$code" in
  0) jq -c '{title, project: .project.name}' "$tmp/document.json" ;;
  6) cat "$tmp/document.err"; jq -e '.error.code == "refused"' "$tmp/document.err" >/dev/null ;;
  *) echo "smoke: document create exited $code" >&2; cat "$tmp/document.err" >&2; exit 1 ;;
esac
"$bin" document get "Smoke document" | jq -c '{title, project: .project.name, content: (.content | length)}' | tee "$tmp/document-get.json"
jq -e '.title == "Smoke document" and .project == "Smoke tests"' "$tmp/document-get.json" >/dev/null
"$bin" document list --project "Smoke tests" | jq -c '{title}' | tee "$tmp/documents.jsonl"
jq -s -e 'map(.title) | index("Smoke document") != null' "$tmp/documents.jsonl" >/dev/null
# A document lives with a project or an issue: Linear refuses one with neither.
"$bin" document create --title "Smoke doc $stamp" --project "Smoke tests" --content "Throwaway, made by scripts/smoke.sh at $stamp." | jq -c '{title, project: .project.name}' | tee "$tmp/document-throwaway.json"
jq -e --arg t "Smoke doc $stamp" '.title == $t and .project == "Smoke tests"' "$tmp/document-throwaway.json" >/dev/null
"$bin" document update "Smoke doc $stamp" --title "Smoky doc $stamp" --content "Rewritten at $stamp." | jq -c '{title, project: .project.name}' | tee "$tmp/document-update.json"
jq -e --arg t "Smoky doc $stamp" '.title == $t and .project == "Smoke tests"' "$tmp/document-update.json" >/dev/null
"$bin" document delete "Smoky doc $stamp" | jq -c '{title, trashed, archivedAt}' | tee "$tmp/document-delete.json"
jq -e '.archivedAt != null' "$tmp/document-delete.json" >/dev/null
"$bin" document unarchive "Smoky doc $stamp" | jq -c '{title, trashed, archivedAt}' | tee "$tmp/document-unarchive.json"
jq -e '.archivedAt == null' "$tmp/document-unarchive.json" >/dev/null
"$bin" document delete "Smoky doc $stamp" | jq -c '{title}' >/dev/null

echo "smoke: muster template create --name 'Smoke template' --type issue --workspace (exit 6 after the first run; it stays, for the fixture), get, list --type issue, then a throwaway team template updated and deleted (ADR 0014)"
printf '{"title": "From the smoke template", "description": "Made by scripts/smoke.sh; the template stays so that the fixture has a node (ADR 0014)."}\n' > "$tmp/template-data.json"
set +e
"$bin" template create --name "Smoke template" --type issue --workspace --data-file "$tmp/template-data.json" --description "The smoke test's own issue template (ADR 0014)." > "$tmp/template.json" 2> "$tmp/template.err"
code=$?
set -e
case "$code" in
  0) jq -c '{name, type, team}' "$tmp/template.json" ;;
  6) cat "$tmp/template.err"; jq -e '.error.code == "refused"' "$tmp/template.err" >/dev/null ;;
  *) echo "smoke: template create exited $code" >&2; cat "$tmp/template.err" >&2; exit 1 ;;
esac
"$bin" template get "Smoke template" | jq -c '{name, type, team, title: (.templateData | fromjson).title}' | tee "$tmp/template-get.json"
jq -e '.name == "Smoke template" and .type == "issue" and .team == null' "$tmp/template-get.json" >/dev/null
"$bin" template list --type issue | jq -c '{name, type, team: .team.key}' | tee "$tmp/templates.jsonl"
jq -s -e 'map(.name) | index("Smoke template") != null' "$tmp/templates.jsonl" >/dev/null
"$bin" template create --name "Smoke template $stamp" --type issue --team SMK --data-file "$tmp/template-data.json" | jq -c '{name, team: .team.key}' | tee "$tmp/template-throwaway.json"
jq -e --arg n "Smoke template $stamp" '.name == $n and .team == "SMK"' "$tmp/template-throwaway.json" >/dev/null
printf '{"title": "Renamed at %s"}\n' "$stamp" > "$tmp/template-data-2.json"
"$bin" template update "Smoke template $stamp" --name "Smoky template $stamp" --data-file "$tmp/template-data-2.json" | jq -c '{name, title: (.templateData | fromjson).title}' | tee "$tmp/template-update.json"
jq -e --arg n "Smoky template $stamp" '.name == $n' "$tmp/template-update.json" >/dev/null
"$bin" template delete "Smoky template $stamp" | jq -c '{success}' | tee "$tmp/template-delete.json"
jq -e '.success == true' "$tmp/template-delete.json" >/dev/null
echo
echo "smoke: muster project get 'Smoke tests', then a throwaway project updated, deleted, unarchived and deleted again (ADR 0014)"
"$bin" project get "Smoke tests" | jq -c '{name, status: .status.name, members: (.members.nodes | length), milestones: (.projectMilestones.nodes | length), trashed}' | tee "$tmp/project-get.json"
jq -e '.name == "Smoke tests"' "$tmp/project-get.json" >/dev/null
# The key's owner stays a member of 'Smoke tests', so that just record-fixtures finds a
# member node to record ProjectByName on (ADR 0014, "Where the fixtures come from").
"$bin" project update "Smoke tests" --add-member me | jq -c '{name}' >/dev/null
"$bin" project create --name "Smoke project $stamp" --description "A throwaway project made by scripts/smoke.sh at $stamp; it ends in the trash." | jq -c '{name}' >/dev/null
"$bin" project update "Smoke project $stamp" --status "In Progress" --lead me --start-date "${stamp%%T*}" --priority 3 --add-member me | jq -c '{name, status: .status.name, lead: .lead.displayName, startDate, priority}' | tee "$tmp/project-update.json"
jq -e '.status == "In Progress" and .priority == 3' "$tmp/project-update.json" >/dev/null
"$bin" project get "Smoke project $stamp" | jq -c '{name, members: [.members.nodes[].displayName]}' | tee "$tmp/project-members.json"
jq -e '(.members | length) == 1' "$tmp/project-members.json" >/dev/null
"$bin" project delete "Smoke project $stamp" | jq -c '{name, trashed}' >/dev/null
"$bin" project list | jq -c 'select(.name == "Smoke project '"$stamp"'")' | tee "$tmp/project-gone.jsonl"
[ ! -s "$tmp/project-gone.jsonl" ] || { echo "smoke: the trashed project is still listed" >&2; exit 1; }
"$bin" project unarchive "Smoke project $stamp" | jq -c '{name}' | tee "$tmp/project-back.json"
jq -e --arg n "Smoke project $stamp" '.name == $n' "$tmp/project-back.json" >/dev/null
"$bin" project delete "Smoke project $stamp" | jq -c '{name}' >/dev/null
echo
echo "smoke: muster milestone create --project 'Smoke tests' --name 'Smoke milestone' (exit 6 after the first run: the duplicate guard; it stays, for the fixture; ADR 0014)"
set +e
"$bin" milestone create --project "Smoke tests" --name "Smoke milestone" --description "The milestone the smoke issues go in; it stays so that the fixture has a node (ADR 0014)." > "$tmp/milestone.json" 2> "$tmp/milestone.err"
code=$?
set -e
case "$code" in
  0) jq -c '{name, status, project: .project.name}' "$tmp/milestone.json" ;;
  6) cat "$tmp/milestone.err"; jq -e '.error.code == "refused" and (.error.message | test("a milestone named"))' "$tmp/milestone.err" >/dev/null ;;
  *) echo "smoke: milestone create exited $code" >&2; cat "$tmp/milestone.err" >&2; exit 1 ;;
esac
"$bin" milestone list --project "Smoke tests" | jq -c '{name, targetDate, status}' | tee "$tmp/milestones.jsonl"
jq -s -e 'map(.name) | index("Smoke milestone") != null' "$tmp/milestones.jsonl" >/dev/null
echo
echo "smoke: a throwaway milestone updated and deleted (ADR 0014)"
"$bin" milestone create --project "Smoke tests" --name "Smoke $stamp" --target-date "${stamp%%T*}" | jq -c '{name, targetDate}' | tee "$tmp/milestone-throwaway.json"
jq -e --arg n "Smoke $stamp" '.name == $n' "$tmp/milestone-throwaway.json" >/dev/null
"$bin" milestone update "Smoke $stamp" --project "Smoke tests" --name "Smoky $stamp" --target-date none | jq -c '{name, targetDate}' | tee "$tmp/milestone-update.json"
jq -e --arg n "Smoky $stamp" '.name == $n and .targetDate == null' "$tmp/milestone-update.json" >/dev/null
"$bin" milestone delete "Smoky $stamp" --project "Smoke tests" | tee "$tmp/milestone-delete.json"
jq -e '.success == true' "$tmp/milestone-delete.json" >/dev/null
echo
echo "smoke: muster project-update create --project 'Smoke tests' --health onTrack, then list (one post per run; ADR 0014)"
"$bin" project-update create --project "Smoke tests" --body "Smoke test at $stamp: the admin commands ran against this workspace (ADR 0014)." --health onTrack | jq -c '{health, user: .user.displayName, project: .project.name}' | tee "$tmp/project-update.json"
jq -e '.health == "onTrack" and .project == "Smoke tests"' "$tmp/project-update.json" >/dev/null
"$bin" project-update list --project "Smoke tests" --limit 3 | jq -c '{health, createdAt}' | tee "$tmp/project-updates.jsonl"
jq -s -e 'length >= 1 and .[0].health == "onTrack"' "$tmp/project-updates.jsonl" >/dev/null
echo
echo "smoke: muster issue create --project 'Smoke tests' --milestone 'Smoke milestone'"
"$bin" issue create --title "Smoke test $stamp" --project "Smoke tests" --milestone "Smoke milestone" --description "Created by scripts/smoke.sh at $stamp. It is canceled by the same run." | tee "$tmp/create.json"
jq -e '.project.name == "Smoke tests" and .projectMilestone.name == "Smoke milestone"' "$tmp/create.json" >/dev/null
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
echo "smoke: muster issue list --milestone 'Smoke milestone'"
"$bin" issue list --milestone "Smoke milestone" | jq -c '{identifier, milestone: .projectMilestone.name}' | tee "$tmp/in-milestone.jsonl"
jq -s -e --arg id "$id" 'map(.identifier) | index($id) != null' "$tmp/in-milestone.jsonl" >/dev/null
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
