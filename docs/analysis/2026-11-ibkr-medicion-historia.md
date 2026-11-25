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

