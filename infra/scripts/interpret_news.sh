#!/bin/sh
# Interpreta las noticias nuevas con Ollama, las guarda y las publica (#186, #191).
# Pensado para cron en el Mac mini; también se puede correr a mano:
#
#   infra/scripts/interpret_news.sh [--limit N]
#
# - Carga el .env de la carpeta del repo (DATABASE_URL, REDIS_URL).
# - PYTHONPATH apunta a intelligence/src: en macOS, Python 3.13 ignora el .pth oculto del venv
#   (docs/guides/puesta-en-marcha-local.md, sección 10).
# - Ollama tiene que estar escuchando en 127.0.0.1:11434 (config/news_interpreter.toml). Si no
#   responde o el digest del modelo no es el fijado, el comando sale con código 2 sin guardar nada.
# - lockf evita dos corridas a la vez: si la anterior sigue (unos 25-35 s por noticia), esta sale
#   sin hacer nada.
# - El registro va a la salida; en cron se redirige a logs/interpret_news.log.
set -eu

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
UV="${UV:-$HOME/.local/bin/uv}"

if [ ! -f "$ROOT/.env" ]; then
  echo "❌ falta $ROOT/.env" >&2
  exit 2
fi
set -a
. "$ROOT/.env"
set +a
export PYTHONPATH="$ROOT/intelligence/src"

mkdir -p "$ROOT/logs"
cd "$ROOT/intelligence"
echo "== $(date -u +%Y-%m-%dT%H:%M:%SZ) interpret_new $*"
exec /usr/bin/lockf -t 0 "$ROOT/logs/interpret_news.lock" \
  "$UV" run -q python -m cfds_intel.news.interpret_new "$@"
