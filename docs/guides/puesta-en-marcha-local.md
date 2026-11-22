# Poner en marcha el sistema en una computadora propia

Esta guía es para correr CFDsTrader en una computadora de ustedes (por ejemplo, el Mac mini), de modo que los datos queden guardados. Los datos que se cargan en una sesión en la nube se pierden cuando la sesión se cierra.

Se puede seguir a mano o pedírsela a un agente que corra **en esa computadora** ("seguí `docs/guides/puesta-en-marcha-local.md`").

> **Reglas que no cambian:** solo la cuenta **demo** de IG (CLAUDE.md §5). El `.env` con las credenciales nunca se commitea ni se pega en un chat. Los datos de IG quedan en la base local y no se suben al repo, que es público.

## 1. Prerrequisitos (una sola vez)

| Herramienta | Para qué | Cómo |
|---|---|---|
| Git | Bajar el repo | Viene con las *Command Line Tools* de macOS: `xcode-select --install` |
| Docker Desktop | PostgreSQL y Redis | docker.com/products/docker-desktop (versión para Apple Silicon en el M4) |
| Rust | El motor | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh`. La versión exacta la toma solo de `rust-toolchain.toml` |
| uv | La capa de Python | `curl -LsSf https://astral.sh/uv/install.sh \| sh` |

## 2. Bajar el repo

En la carpeta elegida (por ejemplo `~/CFDsTrader Mac Mini M4`), vacía:

```bash
cd ~/"CFDsTrader Mac Mini M4"
git clone https://github.com/AlexeiSouyris/CFDsTrader.git .
```

## 3. Completar el `.env` (lo hace una persona)

```bash
cp .env.example .env
open -e .env      # se edita a mano; nunca se pega en un chat
```

- `POSTGRES_USER`, `POSTGRES_PASSWORD` y `POSTGRES_DB`: los elige uno (son locales).
- `DATABASE_URL=postgres://USUARIO:CONTRASEÑA@127.0.0.1:5432/BASE`, con los mismos valores.
- `REDIS_URL=redis://127.0.0.1:6379`.
- `IG_USERNAME`, `IG_PASSWORD` e `IG_API_KEY` de la cuenta **demo** de IG.

Para que los comandos de abajo vean esas variables, en cada terminal nueva:

```bash
set -a; source .env; set +a
```

## 4. Levantar la base y Redis

```bash
docker compose -f infra/docker-compose.yml --env-file .env up -d
docker compose -f infra/docker-compose.yml ps      # los dos tienen que estar "healthy"
```

Los datos quedan en volúmenes de Docker: sobreviven a reinicios de la computadora y de los contenedores.

## 5. Primera compilación y migraciones

```bash
cd engine && cargo build --examples && cd ..
cd intelligence && uv sync && cd ..
```

Las migraciones (`infra/migrations/`) las aplica el motor la primera vez que guarda algo. Por ejemplo, con la carga del paso 6.

Para aplicarlas **sin gastar cupo de IG** (por ejemplo, para cargar primero noticias y COT, que las necesitan): correr una carga diaria **sin** las variables de IG. Migra y frena con `MissingEnv("IG_USERNAME")` antes de conectarse a IG. Si ese día ya estaba guardado, termina con "no falta nada", también sin conectarse.

```bash
cd engine && env -u IG_API_KEY -u IG_USERNAME -u IG_PASSWORD \
  cargo run -q -p cfds-ig --example ig_backfill_daily -- DE40 2021-11-22 2021-11-23; cd ..
```

## 6. Cargas iniciales

El **cupo de IG es de 10.000 puntos por semana**, compartido por todo lo que use la cuenta demo, incluidas las sesiones en la nube. Antes de cada carga grande, probar con un año.

| Qué | Comando | Costo de cupo |
|---|---|---|
| COT de la CFTC (2010 → hoy) | `cd intelligence && uv run python -m cfds_intel.sentiment` | 0 (API pública) |
| €STR del BCE (2019 → hoy), para la financiación del backtest | `cd intelligence && uv run python -m cfds_intel.rates` | 0 (API pública) |
| Noticias de la Fed y del BCE (últimas de cada feed) | `cd intelligence && uv run python -m cfds_intel.news` | 0 |
| Diarias de 5 años, **un año de prueba** | `cd engine && cargo run -p cfds-ig --example ig_backfill_daily -- DE40 2021-11-22 2022-11-22` | ~270 |
| Diarias, el resto | `... ig_backfill_daily -- DE40 2022-11-22 <mañana>` | ~1.060 |
| Ídem para US100, US500 y US30 | Cambiar el instrumento | ~1.330 cada uno |
| Velas de 15m recientes (con su spread) | `cd engine && cargo run -p cfds-ig --example ig_backfill -- DE40 15m <desde> <hasta>` | ~1 por vela |

Notas:
- **Los cuatro índices en diarias** son unos 5.300 puntos: si el cupo no alcanza, se terminan la semana siguiente. Al final de cada carga se muestra el cupo que queda.
- **Si falta un día** de IG, se avisa y **no se rellena**. Por ejemplo, DE40 no tiene el 19-06-2026.
- **Las cargas piden solo lo que falta** (#137). Antes de conectarse a IG muestran cuántas velas cerradas se esperan según el calendario, cuántas ya están guardadas y cuántas se van a pedir. Repetir una carga no gasta cupo por lo ya guardado, y si no falta nada no se conecta. Excepción: un día que IG no tiene (como el del ejemplo) se vuelve a pedir en cada carga que lo incluya, a unos pocos puntos cada vez.
- **El filtro de spread (§7.3)** necesita el spread de los 20 días anteriores al período del backtest. Se carga con `ig_backfill` de 15m.

## 7. Ingestas programadas

Solo graban mientras la computadora está **prendida y despierta**. En macOS: Configuración del Sistema → Batería/Energía → evitar que se suspenda automáticamente.

| Qué | Cada cuánto | Comando |
|---|---|---|
| Noticias de la Fed y del BCE | 15 min | `uv run python -m cfds_intel.news` (en `intelligence/`) |
| Sentimiento de clientes de IG | 15 min | `cargo run -p cfds-ig --example ig_sentiment -- --once` (en `engine/`) |
| COT | 1 vez por semana (por ejemplo, el sábado) | `uv run python -m cfds_intel.sentiment 2026-01-01` (en `intelligence/`) |
| €STR del BCE | 1 vez por día hábil, después de las 09:00 CET | `uv run python -m cfds_intel.rates 2026-01-01` (en `intelligence/`) |
| Velas de 15m y 1h en vivo | Proceso permanente | `cargo run -p cfds-ig --example ig_live` (en `engine/`), unos 3.400 puntos por semana con 4 índices |

**Con `cron`** (`crontab -e`). Reemplazar `RUTA` por la ruta absoluta de la carpeta:

```cron
# En macOS, ver la sección 10 (por qué hace falta esta línea).
PYTHONPATH=RUTA/intelligence/src
*/15 * * * * cd "RUTA/intelligence" && set -a && . ../.env && set +a && $HOME/.local/bin/uv run python -m cfds_intel.news >> ../logs/news.log 2>&1
*/15 * * * * cd "RUTA/engine" && set -a && . ../.env && set +a && $HOME/.cargo/bin/cargo run -q -p cfds-ig --example ig_sentiment -- --once >> ../logs/ig_sentiment.log 2>&1
7 10 * * 6   cd "RUTA/intelligence" && set -a && . ../.env && set +a && $HOME/.local/bin/uv run python -m cfds_intel.sentiment 2026-01-01 >> ../logs/cot.log 2>&1
23 10 * * 1-5 cd "RUTA/intelligence" && set -a && . ../.env && set +a && $HOME/.local/bin/uv run python -m cfds_intel.rates 2026-01-01 >> ../logs/rates.log 2>&1
```

Antes, `mkdir -p "RUTA/logs"`. La carpeta `logs/` no se commitea.

En las variables de `cron` no se expande `$HOME` ni `~`: van rutas absolutas. Si se instala desde un archivo con una ruta larga, `crontab archivo` de macOS puede cortarla; usar `crontab - < archivo`.

En macOS, `cron` puede necesitar **Acceso total al disco**: Configuración → Privacidad y seguridad → Acceso total al disco → agregar `/usr/sbin/cron`.

## 8. Comprobar que anda

```bash
docker compose -f infra/docker-compose.yml exec postgres psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c \
  "select 'velas diarias', count(*) from daily_candles union all
   select 'noticias', count(*) from news_items union all
   select 'sentimiento', count(*) from sentiment_observations;"
tail -n 5 logs/*.log
```

## 9. Si algo falla

- **"Rate limited" o "cupo semanal":** esperar a que se renueve el cupo de IG. No reintentar en bucle.
- **"no empieza a medianoche":** IG cambió la hora de sus velas diarias. No se guarda nada; revisar `docs/research/ig-velas-diarias.md`.
- **Un feed de noticias falla:** los demás siguen. Se reintenta solo en la próxima corrida.
- **"No module named 'cfds_intel'" o `CERTIFICATE_VERIFY_FAILED` en macOS:** ver la sección 10.
- **La computadora estuvo apagada:**
  - **Noticias:** se pierden las que salieron del RSS en ese tiempo, porque un RSS solo guarda las últimas.
  - **Sentimiento de IG:** no hay historia, así que ese tiempo queda sin dato.
  - **Velas:** se recuperan con `ig_backfill`.
  - **COT:** se recupera sin costo.

## 10. Problemas conocidos en macOS

Encontrados al ponerlo en marcha en el Mac mini (M4) el 21 y 22-11-2026.

**`No module named 'cfds_intel'`** aunque `uv sync` terminó bien:
- Causa: macOS marca la carpeta `.venv` como oculta (`ls -lO` muestra `hidden`), y Python 3.13 **ignora los `.pth` ocultos**. El paquete del proyecto se instala justamente con un `.pth`. La marca vuelve a aparecer sola cada tanto.
- Arreglo en una terminal: `chflags -R nohidden intelligence/.venv`.
- Arreglo para `cron`: la línea `PYTHONPATH=RUTA/intelligence/src` del paso 7, que no depende de la marca.

**`CERTIFICATE_VERIFY_FAILED` en los feeds del BCE** (`ecb/press`, `ecb/statpress`), mientras que los de la Fed andan y `curl` también:
- Causa: el Python que instala `uv` usa `/etc/ssl/cert.pem`, al que le falta la raíz de Sectigo que usa `www.ecb.europa.eu`. `curl` usa el llavero de macOS, que sí la tiene.
- Desde #121 **ya no hace falta ningún arreglo**: `intelligence/` verifica los certificados con el almacén del sistema (el llavero de macOS) y nunca desactiva la verificación. Si el error vuelve a aparecer, correr `uv sync` en `intelligence/` para que se instale `truststore`.
- Antes se esquivaba exportando las raíces a un archivo y apuntando `SSL_CERT_FILE` ahí. Esa variable ya no hace falta: se puede sacar del `crontab` y borrar `~/.config/cfdstrader/macos-roots.pem`.

**`docker: command not found`** después de instalar Docker Desktop: abrir la app una vez. Mientras tanto, el binario está en `/Applications/Docker.app/Contents/Resources/bin/`.
