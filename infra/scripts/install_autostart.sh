#!/bin/sh
# Instala el arranque automático de la Mac mini (#863):
# - Docker Desktop como ítem de inicio de sesión.
# - LaunchAgents com.cfdstrader.terminal (gateway + UI) y com.cfdstrader.tailnet (link de Tailscale).
# Correrlo de nuevo los reinstala. Para sacarlos: infra/scripts/install_autostart.sh --remove
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
AGENTS="$HOME/Library/LaunchAgents"
UID_=$(id -u)

for job in terminal tailnet; do
  launchctl bootout "gui/$UID_/com.cfdstrader.$job" 2>/dev/null || true
  rm -f "$AGENTS/com.cfdstrader.$job.plist"
done
if [ "${1:-}" = "--remove" ]; then
  echo "✅ arranque automático sacado (Docker sigue como ítem de inicio)"
  exit 0
fi

# PATH con cargo, node/npx (nvm) y Homebrew tal como están en esta sesión.
NODE_DIR=$(dirname "$(command -v node)")
CARGO_DIR=$(dirname "$(command -v cargo)")
P="$NODE_DIR:$CARGO_DIR:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

mkdir -p "$AGENTS" "$ROOT/logs"
for job in terminal tailnet; do
  sed -e "s#@ROOT@#$ROOT#g" -e "s#@PATH@#$P#g" \
    "$ROOT/infra/launchd/com.cfdstrader.$job.plist" > "$AGENTS/com.cfdstrader.$job.plist"
  plutil -lint "$AGENTS/com.cfdstrader.$job.plist" >/dev/null
  launchctl bootstrap "gui/$UID_" "$AGENTS/com.cfdstrader.$job.plist"
done

osascript -e 'tell application "System Events" to if not (exists login item "Docker") then make login item at end with properties {path:"/Applications/Docker.app", hidden:true}' >/dev/null
echo "✅ arranque automático instalado (logs en logs/autostart_*.log)"
