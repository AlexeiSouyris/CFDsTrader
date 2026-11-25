#!/bin/sh
# Levanta la vista de liquidez en vivo (#231): el gateway del motor (solo lectura) y la UI.
#
#   infra/scripts/ui_liquidez.sh
#
# - Carga el .env de la carpeta del repo (DATABASE_URL) sin mostrarlo.
# - Compila y arranca `cfds-gateway` en la dirección de config/gateway.toml. La sesión de
#   PostgreSQL es de solo lectura: el gateway no puede escribir nada.
# - Arranca `npm run dev` de la UI, que manda /api al gateway, y abre el navegador directo en
#   #liquidez (sin la pantalla de ingreso de la maqueta).
# - Ctrl+C corta los dos.
set -eu

ROOT=$(cd "$(dirname "$0")/../.." && pwd)

if [ ! -f "$ROOT/.env" ]; then
  echo "❌ falta $ROOT/.env (copiar .env.example y completar DATABASE_URL)" >&2
  exit 2
fi
set -a
. "$ROOT/.env"
set +a
if [ -z "${DATABASE_URL:-}" ]; then
  echo "❌ el .env no define DATABASE_URL" >&2
  exit 2
fi

BIND=$(sed -n 's/^bind *= *"\(.*\)"/\1/p' "$ROOT/config/gateway.toml")

echo "▶ compilando el gateway…"
(cd "$ROOT/engine" && cargo build --quiet -p cfds-gateway)

"$ROOT/engine/target/debug/cfds-gateway" &
GATEWAY_PID=$!
trap 'kill "$GATEWAY_PID" 2>/dev/null || true' EXIT INT TERM

# Espera a que el gateway conteste (hasta 30 s).
i=0
until curl -fsS "http://$BIND/api/v1/instruments" >/dev/null 2>&1; do
  i=$((i + 1))
  if [ "$i" -ge 30 ] || ! kill -0 "$GATEWAY_PID" 2>/dev/null; then
    echo "❌ el gateway no arrancó (¿PostgreSQL está levantado?)" >&2
    exit 1
  fi
  sleep 1
done
echo "✅ gateway en http://$BIND (solo lectura)"

cd "$ROOT/ui"
# Instala si falta node_modules o si package-lock.json cambió desde la última instalación (#613).
if [ ! -f node_modules/.package-lock.json ] || [ package-lock.json -nt node_modules/.package-lock.json ]; then
  npm ci
fi
VITE_SKIP_SIGNIN=1 npx vite --open '/#liquidez'
