#!/usr/bin/env bash
# Is this PR commit being run on the EX63 by CI Fast (ci-fast.yml)?
#
# Writes outsider=true|false to $GITHUB_OUTPUT: false when ci-fast.yml routed
# the commit to the EX63 (an org member's PR), true otherwise. Used by
# ci-fast-hosted.yml (run the hosted lanes for outsiders) and ci-lite.yml
# (skip GitHub-hosted CI when the EX63 runs it). ci-gate.yml then requires
# whichever of them ran.
#
# Env: GH_TOKEN, REPO, HEAD_SHA, AUTHOR, ACTOR, ASSOCIATION.
set -euo pipefail
# ci-fast.yml decides who is an org member: it runs from main with
# the CI_MEMBERSHIP_TOKEN secret, so it also sees private members.
# Fork `pull_request` runs get no secrets and see those members only
# as COLLABORATOR, so deciding here as well would run the PR twice.
# Instead, find ci-fast.yml's run for this head commit and read its
# outcome: EX63 lanes skipped means outsider, anything else means
# the EX63 is handling it.
decide_from_ci_fast() {
  local run_id="" lanes=""
  for _ in $(seq 1 36); do
    run_id="$(gh api "repos/${REPO}/actions/workflows/ci-fast.yml/runs?head_sha=${HEAD_SHA}&per_page=5" \
      --jq '[.workflow_runs[] | select(.event == "pull_request_target")] | sort_by(.created_at) | last | .id // empty' 2>/dev/null || true)"
    if [ -n "${run_id}" ]; then
      # A member run materialises "Lanes / CI Fast (EX63)"; an
      # outsider run leaves the skipped call job "Lanes" alone.
      lanes="$(gh api "repos/${REPO}/actions/runs/${run_id}/jobs?per_page=50" \
        --jq 'if any(.jobs[]; .name == "Lanes / CI Fast (EX63)") then "member"
              elif any(.jobs[]; .name == "Lanes" and .conclusion == "skipped") then "outsider"
              else "" end' 2>/dev/null || true)"
      route="$(gh api "repos/${REPO}/actions/runs/${run_id}/jobs?per_page=50" \
        --jq '[.jobs[] | select(.name | startswith("Route"))][0].conclusion // empty' 2>/dev/null || true)"
      if [ "${route}" = "success" ]; then
        case "${lanes}" in
          outsider) echo "true ci-fast run ${run_id}: EX63 lanes skipped"; return 0 ;;
          member) echo "false ci-fast run ${run_id}: EX63 lanes running"; return 0 ;;
          *) ;;  # jobs not materialised yet
        esac
      fi
    fi
    sleep 5
  done
  return 1
}
if verdict="$(decide_from_ci_fast)"; then
  outsider="${verdict%% *}"
  reason="${verdict#* }"
else
  # No ci-fast.yml decision within 3 minutes: fall back to the event's
  # author association (private members may then run here too).
  outsider=true
  if { [ "${ASSOCIATION}" = "OWNER" ] || [ "${ASSOCIATION}" = "MEMBER" ]; } && [ "${ACTOR}" = "${AUTHOR}" ]; then
    outsider=false
  fi
  reason="fallback: no ci-fast.yml decision, association=${ASSOCIATION}"
fi
echo "[ci][route] author=${AUTHOR} actor=${ACTOR} outsider=${outsider} (${reason})"
echo "outsider=${outsider}" >> "$GITHUB_OUTPUT"
