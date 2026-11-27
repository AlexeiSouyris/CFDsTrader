# IBKR como fuente de datos: historia medida y límites (#536)

Medición del 25-11-2026, contra IB Gateway **paper** (127.0.0.1:4002, sesión del dueño), solo lectura,
con `cargo run -p cfds-ibkr --example ibkr_probe` (un pedido por corrida, 11 s entre pedidos,
`client_id` 18). Sin permiso de futuros y **sin ninguna suscripción de datos contratada**. En total
se hicieron unos 35 pedidos de historia: ninguno dio error de ritmo (pacing).

Cómo repetirlo: ver el encabezado de `engine/crates/ibkr/examples/ibkr_probe.rs`. Los instrumentos
salen de `config/ibkr.toml`.

## Resultados

"Velas" = velas **cerradas** devueltas en la ventana pedida (UTC). Horario extendido (`regular_hours_only = false`).

| Instrumento (contrato) | Pedido | Resultado |
|---|---|---|
| MNQ (MNQZ6, vigente) | primera vela (`reqHeadTimeStamp`) | 21-09-2025 22:00 UTC (alta del contrato) |
| MNQ (MNQZ6) | 1m, 24-11-2026 14:00–16:00 | 120 velas |
| MNQ (MNQZ6) | 15m, 24-11-2026 (día entero) | 92 velas (la sesión de CME cierra 1 h por día) |
| MNQ (MNQZ6) | 1h, 24-11-2026 | 23 velas |
| MNQ (MNQZ6) | 1d, 01-09-2025 → 25-11-2026 | 298 velas, desde el 22-09-2025 |
| MNQ (MNQZ6) | 1h, 01-06-2026 | 22 velas |
| MNQ (MNQZ6) | 1h, 22 a 26-09-2025 / 1m, 01-10-2025 | error 162 "HMDS query returned no data" (meses en que el contrato todavía no era el vigente) |
| MES (MESZ6) | primera vela | 21-09-2025 22:00 UTC |
| MES (MESZ6) | 1h, 24-11-2026 / 1m, 24-11-2026 14:00–15:00 | 23 / 60 velas |
| MNQ vencido sep-2026 (MNQU6) | primera vela | 19-06-2025 22:00 UTC |
| MNQ (MNQU6) | 1h, 03-08-2026 / 1m, 03-08-2026 14:00–15:00 | 23 / 60 velas |
| MNQ vencido dic-2024 (MNQZ4) | 1h, 04-11-2024 / 1m, 04-11-2024 15:00–16:00 | 23 / 60 velas |
| MNQ vencido sep-2024 (MNQU4) | 1h, 05-08-2024 | error 200 "No security definition has been found": vencido hace más de 2 años |
| MNQ continuo (CONTFUT) | primera vela | 17-09-2023 22:00 UTC |
| MNQ continuo | 1d, desde 2019 | 813 velas, desde el 18-09-2023 |
| MNQ continuo | 1h, 22 a 25-11-2026 | 47 velas |
| QQQ (ETF, SMART) | primera vela | 10-03-1999 14:30 UTC |
| QQQ | 1m / 15m / 1h, 24-11-2026 | 60 (una hora) / 64 / 16 velas (horario extendido 04:00–20:00 ET) |
| QQQ | 1d, último mes | 22 velas |
| QQQ | 1m, 02-03-2020 15:00–16:00 | 60 velas (6 años atrás) |
| EUR.USD (IDEALPRO, MIDPOINT) | primera vela y 1m, 15m, 1h, 1d | error 162 "No market data permissions for IDEALPRO CASH" |

Un pedido (EUR.USD 1d) no llegó a hacerse: al abrir la sesión, la lectura de las cuentas no respondió
a tiempo y el adaptador cortó (guarda del ADR 0017). Fue una sola vez; la corrida siguiente anduvo.

## Qué significa

1. **Futuros micro por contrato: sirven.** Cada contrato da 1m, 15m, 1h y 1d. Con los vencidos
   (pedidos por mes, `--month AAAAMM`) se llega a unos **2 años hacia atrás** (MNQZ4 sí, MNQU4 no).
   Un contrato tiene datos útiles recién cuando es el más operado; antes, IBKR responde "no data".
   Para una serie larga hay que encadenar contratos (sin mezclar precios de dos contratos en una vela).
2. **El continuo (CONTFUT) no se guarda.** Sus precios están **ajustados** por los vencimientos
   (cierre 17521.69 en 2023, que no es múltiplo del tick de 0.25): cambian en cada nuevo vencimiento,
   o sea que repintan (CLAUDE.md §5b). Además empieza en sep-2023 y no acepta fecha de fin. Sirve solo
   para mirar.
3. **QQQ da historia larga** (1m desde hace al menos 6 años, primera vela en 1999), con la cuenta tal
   como está.
4. **EUR.USD no está disponible** con la cuenta tal como está: falta el permiso de datos de IDEALPRO.
   Si ese permiso es gratis o no, y cómo se activa, es **a confirmar** con IBKR. No se pidió nada.
5. **Diarias (1d):** IBKR las fecha por día de sesión, sin hora. No se guardan en esta etapa (ver
   la migración de velas de IBKR): el motor ya arma sus diarias con su propia ventana.

## Límites oficiales de IBKR

Fuente: "Historical Data Limitations", https://interactivebrokers.github.io/tws-api/historical_limitations.html
(leída el 25-11-2026; la página dice que está deprecada a favor de IBKR Campus). La página nueva,
https://www.interactivebrokers.com/campus/ibkr-api-page/twsapi-doc/#historical-limitations,
respondió 403 a la lectura automática: lo que sigue está **a confirmar** contra ella.

- **Ritmo (pacing), para velas de 30 s o menos:** no repetir un pedido idéntico dentro de 15 s; no
  hacer 6 o más pedidos del mismo contrato, bolsa y tipo de dato en 2 s; no más de **60 pedidos en
  10 minutos**.
- **Velas de 1 minuto o más:** el límite "duro" se levantó, pero sigue uno "blando": pedir demasiado o
  muy seguido puede frenar las respuestas y terminar en la desconexión del cliente de la API.
- **Pedidos abiertos a la vez:** como máximo 50 (se recomienda muchos menos).
- **Duración contra tamaño de vela:** con 1 día de duración se puede pedir de 1 min a 1 día; con
  1 semana, desde 3 min; con 1 mes, desde 30 min; con 1 año, desde 1 día.
- **Historia que no hay:** velas de 30 s o menos de hace más de 6 meses; futuros vencidos hace más
  de **2 años** (coincide con lo medido).
- Sobre el continuo (CONTFUT), `reqHeadTimeStamp` y si la cuenta paper comparte los permisos de datos
  de la real, esa página no dice nada: **a confirmar**.

## Carga incremental (`ibkr_load`)

`cargo run -p cfds-ibkr --example ibkr_load -- ID TAMAÑO [--month AAAAMM] [--from RFC3339] [--run]`
guarda velas cerradas en `ibkr_bars` (`contracts/ibkr_bar.v1.schema.json`, migración 0048). Usa
`[load]` de `config/ibkr.toml`:

- una pausa entre pedidos (11 s: nunca llega a 60 pedidos en 10 minutos) y un tope de pedidos por corrida (30);
- ventanas dentro de la tabla de duraciones (1m: 24 h; 15m: 1 semana; 1h: 30 días);
- sigue desde la última vela guardada de la serie (instrumento, contrato, tamaño y precio); un error
  de ritmo para la corrida (salida 3); "no data" en una ventana (fin de semana, contrato que todavía
  no se operaba) sigue con la siguiente.
- No guarda el continuo (repinta) ni las diarias. No rellena huecos **anteriores** a la última vela
  guardada: para eso, una serie nueva o pedirlo aparte.

Verificación del 25-11-2026: una carga de MNQ (MNQZ6) 1h de 7 días a una base de pruebas propia
(`cfds_test_ibkr2`, no la real): 115 velas nuevas; la segunda corrida no pidió nada (ya estaba al día).

## Precios en vivo y frescura (#541)

Medición del 25-11-2026 a las 01:36 UTC (CME abierto, QQQ cerrado), `client_id` 19, contra MNQZ6, con
4 pedidos en total y una prueba que no quedó en el repo:

| Pedido | Resultado |
|---|---|
| Velas de 1m de los últimos 10 minutos (historia) | 0 velas: la historia llega con demora |
| Snapshot de precios en vivo (`reqMktData`, tipo 1) | error 354: datos no suscriptos |
| Snapshot con datos demorados (tipo 3) | aviso 10167 "Displaying delayed market data"; último, compra, venta, máximo, mínimo, apertura y cierre anterior. Hora del último: 01:26:39 UTC, **unos 10 minutos de demora** |
| Velas de 5 s en vivo (`reqRealTimeBars`) | error 420 "No market data permissions for CME FUT" |

Con la cuenta tal como está, **no hay precios en vivo**: lo más fresco son unos 10 minutos de demora,
igual en la historia de 1 minuto que en los precios demorados. Por eso la terminal `bolsa` usa la
historia de 1 minuto (ya andaba, sin código nuevo en `cfds-ibkr`) y muestra la demora. El cierre
anterior que da el snapshot demorado (26275.75) coincide con la diaria cerrada que usa la terminal
como referencia de la variación del día. Precios en vivo requieren una suscripción de datos de IBKR
(con costo: **a decidir por el dueño**, no se contrató nada).


## Precios por streaming y vela en formación (#558)

Medición del 25-11-2026 entre las 03:18 y las 03:45 UTC (CME abierto; QQQ cerrado, fuera del horario
extendido), con `cargo run -p cfds-ibkr --example ibkr_live` (`client_id` 21, el de `[live]`), contra
IB Gateway paper: seis suscripciones sueltas (`reqMktData` en streaming, sin snapshot; en vivo y
demorado por instrumento) y dos corridas con los tres instrumentos a la vez como los abre el gateway
(`--feed`, la segunda de 7 minutos), unas 18 suscripciones en total. Sin ninguna suscripción de datos
contratada.

| Instrumento (contrato) | Pedido en vivo (`reqMarketDataType` 1) | Pedido demorado (tipo 3) |
|---|---|---|
| MNQ (MNQZ6) | error 354 "Requested market data is not subscribed ... Delayed market data is available" | IBKR informa `delayed`; último precio y su hora cada **~5 s**; atraso medido (llegada − hora del último operado) **600 s** (600-604 s) |
| MES (MESZ6) | error 354 | igual que MNQ: `delayed`, una muestra cada ~5 s, atraso 600-604 s |
| QQQ (NASDAQ.NMS) | error 354 (medido con el mercado cerrado: a repetir en horario de EE. UU., ver abajo) | `delayed`; al suscribirse manda solo el último operado conocido (00:59:59 UTC, 2 h 22 min antes) y nada más hasta que haya operaciones |

- **En vivo no hay**: los tres instrumentos responden 354 al pedir datos en vivo. Con
  `first_try = "live"` el motor lo intenta y, ante ese error, pide demorados solo (lo mismo que ya
  medía #541 para MNQ). El tipo (`delayed`) y el atraso (~600 s) van en cada mensaje.
- **QQQ en horario de EE. UU.**: la medición fue con el mercado cerrado. Si la cuenta tiene la
  suscripción gratuita "US Real-Time Non Consolidated Streaming Quotes", QQQ podría dar `live` en
  horario de mercado. **Pendiente**: repetir `ibkr_live -- QQQ live 30` entre las 14:30 y las 21:00
  UTC. Si da en vivo, el motor ya lo usa sin cambiar código (el mensaje dirá `live`).
- **El último operado previo a la suscripción no es un precio nuevo**: con QQQ cerrado era de horas
  antes y, tomado como precio, armaba una vela vieja y un "atraso" de 8534 s. El motor lo ignora
  (no arma vela ni mide atraso con ese primer par); por eso la primera vela de cada timeframe se
  marca `partial` si su período empezó antes de ese operado.

### La vela en formación al cerrar, contra `ibkr_bars`

Velas de 1m armadas con los precios demorados (corridas de las 03:18 y de las 03:38), contra las
históricas de 1m que guardó `ibkr_load` (lectura de solo lectura de `ibkr_bars`):

| Vela | Armada por streaming (O H L C) | Histórica (O H L C) |
|---|---|---|
| MNQ 03:09 | 25969 · 25972.25 · 25969 · **25969.5** | 25962.5 · 25974 · 25959.25 · **25969.5** |
| MNQ 03:10 | 25971.25 · 26001 · 25971.25 · **25985.25** | 25969.75 · 26003.75 · 25969 · **25985.25** |
| MES 03:11 | 7175 · 7176 · 7174.5 · **7176** | 7173.75 · 7176 · 7172.25 · **7176** |
| MNQ 03:28 (parcial) | 26016 · 26016 · 26008.25 · **26015** | 26021.25 · 26025.75 · 26006.75 · **26015** |
| MES 03:28 (parcial) | 7177.5 · 7177.5 · 7176 · **7177.25** | 7178.25 · 7178.75 · 7175.75 · **7177.25** |
| MNQ 03:29 | **26014** · 26014 · 26002 · **26002** | **26014** · 26015.5 · 25998 · **26002** |
| MES 03:29 | **7177** · 7177 · 7174.75 · **7174.75** | **7177** · 7177.25 · 7174.5 · **7174.75** |

- **El cierre coincidió en las 7**; la apertura en 2 de las 5 no parciales; el máximo y el mínimo
  quedaron siempre **adentro** del rango de la histórica. Es lo esperable: IBKR manda una muestra
  del último precio cada ~5 s (demorados), no cada operado, así que los extremos de la vela pueden
  no verse. Por eso la vela en formación es **solo visual** y la que vale es la histórica.
- Con todos los precios (un test con las velas de 1m reales de MNQZ6 de 02:00 a 02:14), la vela
  armada es exactamente la histórica de 1m y su agregado de 5m y 15m
  (`engine/crates/ibkr/tests/live_coherence.rs`).
- **Problema aparte encontrado**: las velas de 15m y 1h que guardó `ibkr_load` en las cargas
  incrementales del 25-11 (por ejemplo MNQ 15m 02:00) **no coinciden** con el agregado de sus velas
  de 1m: se guardaron antes de que IBKR tuviera todos sus datos (el atraso de ~10 minutos). Ver #563.

Cómo repetirlo: ver el encabezado de `engine/crates/ibkr/examples/ibkr_live.rs`.
