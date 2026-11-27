# Ver la bolsa desde la terminal (`bolsa`, #541)

Una pantalla de terminal de trading, **solo lectura y solo presentación**, con datos de IBKR paper:

- **Watchlist** a la izquierda: último precio, variación del día (%) y hora de la última vela.
- **Gráfico de velas** grande a la derecha, del instrumento elegido, con escala de precios y de tiempo
  y el timeframe marcado (1m, 15m, 1h, 1d).
- **Panel del instrumento** debajo de la watchlist: contrato, último, variación del día, cierre de la
  sesión anterior, hora y demora, rango y volumen del día, y apertura, máximo, mínimo, cierre y volumen
  de la última vela cerrada del gráfico.
- **Barra de estado** abajo: conexión con IB Gateway (o kill switch), fuente, demora de los datos,
  próximo pedido y teclas.

No calcula indicadores ni señales y no envía órdenes (ni paper). Ningún texto promete ganancias.

## Cómo correrlo

Antes: IB Gateway abierto en esta Mac con login **paper** hecho a mano (127.0.0.1:4002) e
`IBKR_PAPER_ACCOUNT_ID` en `.env`.

```sh
cd ~/Documents/"CFDsTrader Home"/engine
set -a; . ../.env; set +a
cargo run -q --release -p cfds-terminal --bin bolsa                 # pantalla completa
cargo run -q --release -p cfds-terminal --bin bolsa -- --once       # la tabla una vez, como texto
cargo run -q --release -p cfds-terminal --bin bolsa -- --check-config
```

(La primera vez compila unos minutos.) Teclas: **↑/↓** (o `k`/`j`, o `1`–`9`) instrumento,
**←/→** (o `h`/`l`, Tab) timeframe, **q** / Esc / Ctrl+C salir. Sin colores: `--no-color` o
`NO_COLOR=1` (la variación igual lleva signo y las velas bajistas se dibujan con `░`). Se ve bien desde
unas 80 columnas; con menos de 77 los paneles van uno debajo del otro.

Salida: 0 bien; 1 configuración inválida o no se pudo conectar; 2 kill switch (cuenta que no es paper,
desconexión o error de datos), con alerta como `ibkr_read`.

## De dónde salen los precios y qué tan frescos son

- **Velas históricas cerradas** por la sesión de solo lectura de `cfds-ibkr` (guardas de puerto,
  cuenta paper y kill switch), con `client_id` 19 (`config/terminal.toml`). El último precio es el
  cierre de la última vela de **1 minuto** cerrada.
- La cuenta no tiene suscripción de datos: lo medido el 25-11-2026 es que la historia y los precios
  "en vivo" llegan con **unos 10 minutos de demora** (ver
  `docs/analysis/2026-11-ibkr-medicion-historia.md`, sección "Precios en vivo"). La pantalla muestra la
  demora de cada precio.
- **Variación del día** = último precio contra el cierre de la diaria de la **sesión anterior** a la
  del último precio (el día de sesión se calcula con el corrimiento de `[session]`). Sin velas de 1
  minuto en las últimas 24 h (fin de semana), muestra el último cierre diario contra el anterior.

## Ritmo de pedidos

Un solo pedido a la vez y nunca dos más cerca que `refresh.min_request_gap_secs` (15 s: como mucho
40 pedidos en 10 minutos; el piso es 11 s). Cada instrumento pide su vela de 1 minuto cada
`quote_secs` (60 s) y sus diarias cada `daily_secs` (30 min); el gráfico que se mira, cada `chart_secs`
(60 s). Al cambiar de gráfico, el nuevo va primero. Si corre a la vez una carga `ibkr_load` (otro
`client_id`), los dos suman pedidos: para velas de 1 minuto o más IBKR aplica un límite "blando".

## Límites

- Datos con demora (~10 min) mientras no haya suscripción de datos de IBKR.
- EUR/USD no se muestra: falta el permiso de datos de IDEALPRO.
- El gráfico de futuros es del contrato vigente (sin empalmar vencimientos).
- Posiciones y órdenes de la cuenta paper del motor: no están en esta pantalla (otro Issue).
