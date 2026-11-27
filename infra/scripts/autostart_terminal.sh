#!/bin/sh
# Arranque automático de la terminal (#863). Lo corre launchd (infra/launchd/) al iniciar sesión.
# Espera a que Docker levante PostgreSQL y Redis, y después corre ui_liquidez.sh sin abrir el
# navegador. Si algo se cae, launchd lo vuelve a correr.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)

i=0
until nc -z 127.0.0.1 5432 2>/dev/null && nc -z 127.0.0.1 6379 2>/dev/null; do
  i=$((i + 1))
  if [ "$i" -ge 300 ]; then
    echo "❌ PostgreSQL o Redis no respondieron en 10 minutos (¿Docker abierto?)" >&2
    exit 1
  fi
  sleep 2
done
echo "✅ PostgreSQL y Redis listos"
CFDS_NO_OPEN=1 exec sh "$ROOT/infra/scripts/ui_liquidez.sh"
