#!/bin/zsh
# Uso: infra/scripts/merge_pr.sh N
# Mergea con squash el PR N solo si (1) su rama está al día con main y (2) el check "CI ok"
# pasó sobre ese último commit. Si la rama está atrasada, la actualiza con main y espera el CI
# nuevo: así dos PRs verdes por separado no rompen main juntos (#584). Pensado para el agente
# del dueño (delegación de merge, CLAUDE.md §2).
set -u
n=$1
repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
gh pr edit "$n" --base main >/dev/null 2>&1
deadline=$(( $(date +%s) + 45*60 ))
while [ "$(date +%s)" -lt "$deadline" ]; do
  st=$(gh pr view "$n" --json state -q .state)
  [ "$st" = MERGED ] && { echo "#$n merged"; exit 0; }
  [ "$st" = CLOSED ] && { echo "#$n closed"; exit 5; }
  head=$(gh pr view "$n" --json headRefName -q .headRefName)
  behind=$(gh api "repos/$repo/compare/main...$head" --jq .behind_by 2>/dev/null || echo 0)
  if [ "$behind" -gt 0 ]; then
    if ! gh pr update-branch "$n" >/dev/null 2>&1; then echo "#$n DIRTY (conflicto con main)"; exit 3; fi
    sleep 30; continue
  fi
  sha=$(gh pr view "$n" --json headRefOid -q .headRefOid)
  ok=$(gh api "repos/$repo/commits/$sha/check-runs?check_name=CI%20ok" --jq '.check_runs[0] | "\(.status) \(.conclusion)"' 2>/dev/null)
  case "$ok" in
    "completed success")
      gh pr merge "$n" --squash --match-head-commit "$sha" >/dev/null 2>&1 && { echo "#$n merged"; exit 0; }
      sleep 15 ;;
    completed\ *) echo "#$n CI falló ($ok)"; exit 4 ;;
    *) sleep 30 ;;
  esac
done
echo "#$n timeout esperando CI"; exit 6
