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
- `CFDS_DATA_MACHINE=1`, **solo en la computadora que guarda los datos** (el Mac mini). Sin esa línea, cualquier carga de IG de más de 100 puntos se niega (#143).

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

Antes de cargar, `--dry-run` muestra qué falta y cuántos puntos estima, sin conectarse a IG. Una carga que estime más de 500 puntos (`loads.confirm_above_points` en `config/ig.toml`) pregunta antes de seguir, salvo que se agregue `--yes`. Sin terminal, por ejemplo desde `cron`, se niega.

| Qué | Comando | Costo de cupo |
|---|---|---|
| COT de la CFTC (2010 → hoy) | `cd intelligence && uv run python -m cfds_intel.sentiment` | 0 (API pública) |
| €STR del BCE (2019 → hoy), para la financiación del backtest | `cd intelligence && uv run python -m cfds_intel.rates` | 0 (API pública) |
| Noticias de la Fed y del BCE (últimas de cada feed) | `cd intelligence && uv run python -m cfds_intel.news` | 0 |
| Interpretación de las noticias nuevas con Ollama (ADR 0008, #186): guarda en `news_interpretations` y publica en Redis. Necesita Ollama con el modelo fijado; ~25 s por noticia. `--limit N` para ir de a poco. Para `cron`, ver la sección 7 | `infra/scripts/interpret_news.sh [--limit N]` (desde la raíz del repo) | 0 (no usa IG) |
| Diarias de 5 años, **un año de prueba** | `cd engine && cargo run -p cfds-ig --example ig_backfill_daily -- DE40 2021-11-22 2022-11-22` | ~270 |
| Diarias, el resto | `... ig_backfill_daily -- DE40 2022-11-22 <mañana>` | ~1.060 |
| Indicadores de las diarias (ATR diario y volatilidad realizada, #148; caída desde el máximo, #213), **después de cada carga de diarias** | `cd engine && cargo run -p cfds-ingest --example daily_indicators` | 0 (no usa IG) |
| Mapa de liquidez (spec §5.4.1, #231, solo informativo), **después de cada carga de velas de 15m**. Es idempotente y solo agrega los niveles y estados nuevos | `cd engine && cargo run -p cfds-ingest --example liquidity_levels` | 0 (no usa IG) |
| Estadísticas de liquidez (spec §5.4.1, #231, solo informativas): eventos de FVG de 15m, 1h y 1d, gaps de apertura, desequilibrio y agregados, **después de cada carga de velas de 15m o diarias**. Es idempotente: solo agrega los eventos nuevos, y los agregados solo si cambiaron | `cd engine && cargo run -p cfds-ingest --example liquidity_stats` | 0 (no usa IG) |
| Índice de sentimiento v1 (spec §9.1, #213), **después de los indicadores de las diarias o de cargar el COT** | `cd intelligence && uv run python -m cfds_intel.sentiment.compute_index` | 0 (no usa IG ni la red) |
| Análogos v1 de la última ventana de cada índice (#148, solo informativo) | `cd intelligence && uv run python -m cfds_intel.analogs [INSTRUMENTO]` | 0 (no usa IG) |
| Ídem para US100, US500 y US30 | Cambiar el instrumento | ~1.330 cada uno |
| Verificación de las velas de US500, US100 y US30 contra SPY, QQQ y DIA de Tiingo (ADR 0009, #206). Guarda solo el veredicto de cada día; `--dry-run` no guarda nada; `--calibrate` muestra cuántos días coinciden con cada tolerancia candidata. Necesita `TIINGO_API_KEY` y `CFDS_DATA_MACHINE=1` | `cd engine && cargo run -q -p cfds-tiingo --example tiingo_verify -- 2026-10-01 2026-10-31` | 0 de IG; 1 pedido de Tiingo por ETF (topes en `config/tiingo.toml`) |
| Histórico de 1 minuto de futuros desde Databento (ADR 0011, **en propuesta**; #247). Primero `--estimate` (gratis: costo por símbolo y total; frena si pasa `max_usd` de `config/databento.toml`) y `--sample` (un día por símbolo, validado). Después `--run`, que solo corre con `CFDS_DATA_MACHINE=1` y sigue desde lo ya bajado. Necesita `DATABENTO_API_KEY` | `cd intelligence && uv run python -m cfds_intel.databento --estimate` | 0 de IG; crédito de Databento (USD 125 gratis) |
| Velas de 15m recientes (con su spread) | `cd engine && cargo run -p cfds-ig --example ig_backfill -- DE40 15m <desde> <hasta>` | ~1 por vela |

Notas:
- **Los cuatro índices en diarias** son unos 5.300 puntos: si el cupo no alcanza, se terminan la semana siguiente. Al final de cada carga se muestra el cupo que queda, cuándo se renueva, lo gastado y lo **gastado por otro lado** desde la carga anterior registrada en esta base (#140).
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
| Índice de sentimiento (#213). **Lo agrega el dueño cuando decida** | 1 vez por día hábil, después de los indicadores de las diarias, y después del COT del sábado | `uv run python -m cfds_intel.sentiment.compute_index` (en `intelligence/`) |
| Calendario económico de Forex Factory por su feed JSON oficial (ADR 0010, #239). **Lo agrega el dueño cuando decida** | Cada 4 horas, a un minuto raro (el archivo cambia una vez por hora y pedirlo más seguido da 429) | `uv run python -m cfds_intel.calendar` (en `intelligence/`) |
| €STR del BCE | 1 vez por día hábil, después de las 09:00 CET | `uv run python -m cfds_intel.rates 2026-01-01` (en `intelligence/`) |
| Velas de 15m y 1h en vivo (#124) | 15 min, un minuto después de cada cierre (`:01`, `:16`, `:31`, `:46`) | `cargo run -q -p cfds-ig --example ig_live -- --once` (en `engine/`). Gasta unos 690 puntos por semana con 4 índices, de los 3.400 de `budget.live` |
| Interpretación de las noticias nuevas con Ollama (#186, #191). **La agrega el dueño cuando decida** | 15 min, 3 minutos después de la ingesta de noticias (`:03`, `:18`, `:33`, `:48`) | `infra/scripts/interpret_news.sh --limit 20` (desde la raíz del repo) |

**Con `cron`** (`crontab -e`). Reemplazar `RUTA` por la ruta absoluta de la carpeta:

```cron
# En macOS, ver la sección 10 (por qué hace falta esta línea).
PYTHONPATH=RUTA/intelligence/src
*/15 * * * * cd "RUTA/intelligence" && set -a && . ../.env && set +a && $HOME/.local/bin/uv run python -m cfds_intel.news >> ../logs/news.log 2>&1
*/15 * * * * cd "RUTA/engine" && set -a && . ../.env && set +a && $HOME/.cargo/bin/cargo run -q -p cfds-ig --example ig_sentiment -- --once >> ../logs/ig_sentiment.log 2>&1
7 10 * * 6   cd "RUTA/intelligence" && set -a && . ../.env && set +a && $HOME/.local/bin/uv run python -m cfds_intel.sentiment 2026-01-01 >> ../logs/cot.log 2>&1
# Calendario de Forex Factory (ADR 0010): cada 4 horas, nunca más de una vez por hora.
17 */4 * * * cd "RUTA/intelligence" && set -a && . ../.env && set +a && $HOME/.local/bin/uv run python -m cfds_intel.calendar >> ../logs/calendar.log 2>&1
23 10 * * 1-5 cd "RUTA/intelligence" && set -a && . ../.env && set +a && $HOME/.local/bin/uv run python -m cfds_intel.rates 2026-01-01 >> ../logs/rates.log 2>&1
1,16,31,46 * * * * cd "RUTA/engine" && set -a && . ../.env && set +a && /usr/bin/lockf -t 0 ../logs/ig_live.lock $HOME/.cargo/bin/cargo run -q -p cfds-ig --example ig_live -- --once >> ../logs/ig_live.log 2>&1
3,18,33,48 * * * * "RUTA/infra/scripts/interpret_news.sh" --limit 20 >> "RUTA/logs/interpret_news.log" 2>&1
```

Antes, `mkdir -p "RUTA/logs"`. La carpeta `logs/` no se commitea.

**Velas en vivo (`ig_live --once`, #124).** Cómo funciona cada corrida del `cron`:

- **Qué pide:** retoma desde la última vela guardada en la base y pide solo las velas de 15m y 1h que cerraron desde entonces, en horario de sesión. Si la última vela guardada es de hace más de `live.startup_lookback_hours` (24 h), no va más atrás: los huecos viejos se completan con `ig_backfill`.
- **Cuándo corre:** las velas de 15m cierran en `:00`, `:15`, `:30` y `:45`, y las de 1h en `:00`. Se piden 20 s después del cierre (`live.candle_delay_secs`). El `cron` trabaja de a minutos, así que corre un minuto después del cierre.
- **Corridas superpuestas:** `lockf -t 0` evita que se pisen. Si la anterior sigue corriendo, por ejemplo compilando después de un `git pull`, la nueva sale sin hacer nada.
- **Costo:** una vela es un punto. Por día hábil son 138: 26 velas de 15m y 6 de 1h por cada índice de EE. UU., y 34 y 8 para DE40. Eso da unas **690 por semana** contra los 3.400 de `budget.live`. Un test lo controla con el calendario real (`a_week_of_live_candles_fits_the_live_budget`). Se suman los reintentos y lo que haya que completar después de un apagón, hasta 24 h.
- **Si se agota el presupuesto:** antes de cada pedido se controla que entre en `budget.live` y que no toque `budget.reserve` (#167). Si no entra, la corrida termina con `kill switch: … presupuesto semanal de IG …` en `logs/ig_live.log`, **sin pedir nada**. Las corridas siguientes hacen lo mismo, sin gastar, hasta que IG renueve el cupo. Ahí siguen solas desde la última vela guardada, hasta 24 h atrás.
- **Si una vela no llega:** la corrida avisa y la siguiente la vuelve a pedir. Si pasan 10 minutos (`live.give_up_after_secs`), la corrida termina con `kill switch` y la siguiente vuelve a intentar. Cada intento puede gastar puntos si IG devuelve velas que no validan. Por eso conviene mirar el log, y el presupuesto pone el techo.
- **Antes de prenderlo:** `cargo run -q -p cfds-ig --example ig_live -- --dry-run` (en `engine/`, con el `.env` cargado) muestra qué pediría, cuántos puntos y cómo está el presupuesto, sin conectarse a IG.

**Interpretación de noticias (`interpret_news.sh`, #191).** Interpreta con Ollama las noticias de `news_items` que todavía no tienen interpretación del intérprete fijado, las guarda en `news_interpretations` y las publica en Redis (#186):

- **Antes de prenderlo:** Ollama tiene que estar corriendo y escuchando solo en `127.0.0.1:11434`, que es lo que pide `config/news_interpreter.toml`. Hoy en el Mac mini se arranca a mano con `ollama serve`, así que no sobrevive a un reinicio. Para que arranque solo: `brew services start ollama`, que por defecto también escucha solo en `127.0.0.1`. Lo decide el dueño.
- **Probarlo a mano:** `infra/scripts/interpret_news.sh --limit 2`, desde la raíz del repo. El script carga el `.env` y fija `PYTHONPATH` (sección 10).
- **Tiempo:** unos 25 a 35 s por noticia, porque el modelo se carga y se descarga en cada una (`keep_alive = 0`). Con `--limit 20`, una corrida dura menos de 15 minutos. La primera vez hay atraso: se va poniendo al día de a 20 por corrida.
- **Corridas superpuestas:** `lockf -t 0` evita que se pisen. Si la anterior sigue, la nueva sale con `already locked` y código 75, sin hacer nada.
- **Si Ollama no responde o el digest del modelo no es el fijado:** sale con código 2 y un mensaje con ❌, sin guardar nada.
- **Si se caen Ollama, PostgreSQL o Redis en el medio:** corta ahí. Lo ya guardado queda, y la próxima corrida sigue.
- **Si una noticia no se puede leer** (su página da error) **o la respuesta del modelo no cumple las reglas:** se avisa con ⚠️, se sigue con las demás y sale con código 1. Esa noticia se vuelve a intentar en cada corrida.
- **Memoria:** el modelo ocupa unos 5,4 GB mientras interpreta y se libera al terminar cada noticia.

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

**Interpretaciones de noticias, de Python a Rust (#201).** Después de una corrida de `infra/scripts/interpret_news.sh`, comprobar que el motor las recibe por Redis:

```bash
cd engine && set -a && . ../.env && set +a && cargo run -q -p cfds-storage --example news_stream_tail -- --from-start --once
```

Muestra cada interpretación nueva: hora, fuente, intérprete, resumen y sentimiento y relevancia por instrumento. Usa su propio grupo de lectura y no le quita nada al motor. Una segunda corrida muestra solo lo nuevo. Sin `--once`, se queda esperando las próximas.

**Vista de liquidez en vivo (#231).** Con PostgreSQL levantado:

```bash
infra/scripts/ui_liquidez.sh
```

Carga el `.env` sin mostrarlo, compila y arranca el gateway (`cfds-gateway`, solo lectura: la sesión de PostgreSQL no puede escribir) en la dirección de `config/gateway.toml`, y abre la UI directo en `#liquidez`. Arriba se elige el instrumento (los del catálogo) y el timeframe (15m, 1h o 1d); abre en lo que diga `[view]` de `config/gateway.toml`. Sin velas guardadas de ese timeframe muestra "Sin datos todavía". `Ctrl+C` corta todo. Para ver solo la respuesta: `curl 'http://127.0.0.1:8787/api/v1/liquidity?instrument=DE40&timeframe=1d'`.

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
