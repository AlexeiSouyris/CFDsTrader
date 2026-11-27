# Trofeos de proceso: reglas medibles (propuesta)

> **Estado: propuesta (#651).** Solo documentación: no hay código, config ni contratos nuevos en
> este PR. Cada umbral es un **parámetro PENDIENTE** del dueño (tabla «Puntos para el dueño», §5).
> Nada de esto se implementa hasta que el dueño lo apruebe en el Issue o en el PR.

## 1. Para qué

La vista **Trofeos** (`#competencia`) muestra hoy 5 trofeos de proceso con el estado «sin datos»
(`ui/src/lib/trophy-rules.ts`, #616 y #622): la UI tiene el catálogo, pero nadie calcula si se
cumplieron. Este documento propone **una definición objetiva para cada trofeo**, calculable solo con
datos que el motor ya guarda o va a guardar, para que el cálculo se haga más adelante **en el
motor** y la UI solo lo muestre (CLAUDE.md §1).

## 2. Reglas para todos los trofeos

1. **Premian proceso, nunca ganancia.** Ningún trofeo usa `gross_pnl`, `net_pnl`, la tasa de
   aciertos, `hit_tp1` ni el saldo. Ningún texto habla de ganar (CLAUDE.md §5;
   `docs/design/2026-11-rediseno-ui.md`, Trofeos). Del resultado (`outcome.v1`) solo se usan horas
   y motivos de salida.
2. **Solo paper.** Se cuentan propuestas, órdenes y resultados con `source = paper` /
   `kind = simulated`. Los de backtest no cuentan.
3. **Actividad mínima, para no premiar no operar.** Un período con menos actividad que la mínima de
   ese trofeo queda **«sin actividad»**: ni logrado ni fallado. Así, no operar nunca da un trofeo.
4. **Estados:** `logrado`, `no logrado`, `en curso` (el período todavía no cerró), `sin actividad`
   y `sin datos` (faltan los datos para decidir, por ejemplo calendario caído o un dato que el motor
   todavía no guarda). La UI ya sabe mostrar «sin datos».
5. **Sin repintado.** El estado de un período cerrado no cambia nunca. Se calcula con los registros
   tal como estaban al cierre del período (todos son de solo agregar y tienen hora).
6. **Períodos.** Las horas se guardan en UTC; el corte del día, la semana (lunes a domingo) y el mes
   se hace en la zona del límite de pérdida diaria (hoy 00:00 de Madrid, `config/risk.toml`), para
   que «día» signifique lo mismo en el riesgo y en los trofeos.
7. **Trofeos que el sistema ya garantiza.** Si el motor ya obliga la regla (por ejemplo, el runner
   no entra cerca de una noticia de alto impacto), el trofeo no mide a la persona. En esos casos se
   cuentan **solo los tickets manuales** (`order.v1` con `proposal_id = null`, por
   `POST /api/v1/paper/orders`) o se marca el trofeo como «lo garantiza el sistema». Se dice en cada
   trofeo.
8. **Rechazos que no son de la persona** no cuentan en contra: `no_price`, `no_recent_candles`,
   `missing_fx_rate`, `unknown_instrument`, `min_deal_size_unknown` (datos o config).

## 3. Los 5 trofeos de hoy

### 3.1 Stop respetado (`stop`, semana)

- **Regla:** en la semana, ninguna posición tuvo un cambio de stop que lo **alejara** de la entrada
  (agrandar el riesgo). Acercarlo, pasarlo a break-even o el trailing del motor están permitidos.
- **Actividad mínima:** `min_closed_positions` posiciones cerradas en la semana.
- **Datos:** hoy el broker simulado **no permite mover el stop** (el stop es obligatorio y fijo en
  `order.v1`), así que hoy la regla se cumple siempre: es un trofeo **que garantiza el sistema**.
  Mide algo recién cuando exista editar el stop; ahí hace falta un registro de cada cambio
  (contrato futuro `position_stop_change.v1`: posición, stop anterior, stop nuevo, quién, cuándo).
- **Hasta entonces:** mostrarlo como «lo garantiza el sistema», no como logrado.
- **Trampa a evitar:** no operar para no fallar → la actividad mínima lo frena.

### 3.2 Diario completo (`diario`, semana)

- **Regla:** cada operación cerrada en la semana tiene una nota escrita **antes del corte del día**
  en que cerró (la hora de la última salida de `outcome.v1`).
- **Nota válida:** al menos `min_note_chars` caracteres y distinta de las otras notas de la semana
  (no vale copiar la misma).
- **Actividad mínima:** 1 operación cerrada.
- **Datos:** hoy **no existen** las notas del diario (`gateway_journal.v1` solo junta propuesta y
  resultado). Hace falta un contrato futuro `journal_note.v1` (propuesta, texto, `written_at`), de
  solo agregar. Hasta entonces: «sin datos».
- **Trampa a evitar:** notas vacías o repetidas → largo mínimo y sin duplicados. La calidad de la
  nota no es medible y no se intenta medir.

### 3.3 Dentro del límite (`limite`, mes)

- **Regla:** en el mes no hubo ningún rechazo `daily_loss_limit` ni `drawdown_limit`, ni una
  activación del kill switch por drawdown (`paper_kill_switch`).
- **Actividad mínima:** `min_active_days` días del mes con al menos una orden enviada.
- **Datos:** existen: el registro de órdenes (`order_report.v1`, motivo de rechazo) y la tabla
  `paper_kill_switch` (#441). Se cuentan los rechazos de órdenes del runner y de tickets manuales.
- **Trampa a evitar:** no operar en todo el mes → actividad mínima. No se muestra ningún monto de
  pérdida en el trofeo: solo si el límite se tocó o no.

### 3.4 Propuestas a tiempo (`propuestas`, día)

- **Regla:** cada propuesta del día recibió una decisión de la persona (aprobar o rechazar) antes de
  vencer. Una propuesta vence a los `proposal_ttl_minutes` de `signal_at` (después ya no se ejecuta
  en `t+1`, CLAUDE.md §5b).
- **Actividad mínima:** 1 propuesta en el día.
- **Datos:** hoy el runner de paper **manda las órdenes solo**, sin aprobación de una persona
  (`config/runner.toml`), así que no hay decisión que medir: «sin datos». Hace falta un modo con
  aprobación y un contrato futuro `proposal_decision.v1` (propuesta, `approved | rejected |
  expired`, `decided_at`).
- **Trampa a evitar:** aprobar todo sin mirar → una decisión tomada en menos de
  `min_review_seconds` desde que se mostró no cuenta como «a tiempo». Rechazar también es una
  decisión válida: el trofeo no empuja a operar más.

### 3.5 Calendario revisado (`calendario`, semana)

- **Regla:** ninguna entrada de la semana se ejecutó dentro de la ventana de una noticia de impacto
  alto del instrumento o de su país, usando solo eventos ya publicados a la hora de la entrada
  (`published_at ≤ entry_at`).
- **Ventana:** la misma del filtro de noticias, `config/news_filter.toml [news] window_minutes`
  (hoy 15) e impactos `blocking_impacts`. **No se duplica el número**: el trofeo lee esa config.
  El texto de hoy en la UI dice «15 minutos antes»; se propone alinearlo con el filtro (antes y
  después).
- **Actividad mínima:** `min_entries_week` entradas de tickets manuales.
- **Datos:** existen: entradas (`outcome.v1 entry_at`, o el fill de `order_report.v1`) y el
  calendario (`economic_event.v1`). El runner ya aplica este filtro (§7.2), así que **solo se
  cuentan los tickets manuales**. Si el calendario estaba caído en la semana
  (`calendar_stale_after_days`), el estado es «sin datos», no logrado.
- **Pregunta abierta:** ¿cuenta también **mantener** una posición abierta durante el evento? Se
  propone que no (la regla de la spec habla de abrir).

## 4. Propuesta de 5 trofeos nuevos

### 4.1 Pausa respetada (`pausa`, mes)

- **Regla:** mientras el kill switch estuvo activo, no se intentó ningún ticket manual (cero
  rechazos `kill_switch_active` de órdenes con `proposal_id = null`), y la liberación quedó
  registrada como liberación (`paper_kill_switch`, `change = released`) y no a mano en la base.
- **Actividad mínima:** al menos una activación del kill switch en el mes; si no hubo, «sin
  actividad».
- **Datos:** existen (`paper_kill_switch`, registro de órdenes).
- **Trampa a evitar:** provocar activaciones para tener el trofeo → solo se muestra como logrado,
  nunca suma puntos ni rachas extra; y una activación ya es una señal de problema que se revisa.

### 4.2 Tickets limpios (`tickets`, semana)

- **Regla:** ningún ticket manual de la semana fue rechazado por el riesgo por un error de armado:
  `invalid_input`, `stop_wrong_side`, `stop_too_close`, `stop_too_far`, `insufficient_margin`,
  `below_min_deal_size`, `max_open_positions`, `instrument_already_open`.
- **Actividad mínima:** `min_manual_tickets` tickets manuales enviados.
- **Datos:** existen (`order_report.v1`). La vista previa (`ticket_verdict.v1`) no se guarda, así
  que usarla antes de enviar es justamente lo que el trofeo premia, sin castigar las vistas previas
  rechazadas.
- **Trampa a evitar:** mandar pocos tickets para no fallar → actividad mínima.

### 4.3 Preregistro primero (`preregistro`, mes)

- **Regla:** cada fila nueva del mes en `docs/analysis/registro-de-resultados.csv` que sea una
  prueba (backtest, walk-forward, ronda o ML) tiene un preregistro (documento con criterio escrito
  antes) cuyo commit en `main` es **anterior** al commit de la fila.
- **Actividad mínima:** 1 fila nueva en el mes.
- **Datos:** existen en el repo (registro de resultados y `docs/analysis/*preregistro*.md`). Falta
  una columna que enlace la fila con su preregistro: se propone agregarla al registro en el PR que
  implemente el trofeo (fila nueva, nunca editar las viejas).
- **Trampa a evitar:** un preregistro vacío escrito minutos antes → mínimo `min_prereg_lead_hours`
  entre el commit del preregistro y el de la fila.

### 4.4 Cambios votados (`cambios`, mes)

- **Regla:** todo cambio de parámetros de estrategia o riesgo aplicado en el mes pasó antes por una
  propuesta de cambio y su votación (`change_proposal.v1`, `change_vote_status.v1`) con resultado
  aprobado.
- **Actividad mínima:** 1 cambio aplicado en el mes.
- **Datos:** existen las propuestas y los votos (`config/votes.toml`). Falta saber **cuándo se
  aplicó** cada cambio a la config: se propone tomarlo del hash de la config que ya guarda cada
  corrida, o pedir el dato en el PR que lo implemente.
- **Trampa a evitar:** no cambiar nunca nada para no fallar → actividad mínima.

### 4.5 Racha de proceso (`racha`, semanas seguidas)

- **Regla:** `streak_weeks` semanas seguidas en las que todos los trofeos semanales que tuvieron
  actividad quedaron logrados. Una semana «sin actividad» no corta la racha ni la suma.
- **Datos:** sale de los estados de los otros trofeos; no lee datos nuevos.
- **Trampa a evitar:** que la racha empuje a no operar → las semanas sin actividad no suman.

## 5. Puntos para el dueño (todos PENDIENTE)

| Parámetro | Trofeo | Recomendado | Por qué | Estado |
|---|---|---|---|---|
| `period_timezone` | todos | `Europe/Madrid` | El mismo corte del límite de pérdida diaria (`config/risk.toml`). | PENDIENTE |
| `min_closed_positions` | stop | 3 por semana | Con menos, cumplir es casi seguro y no dice nada. | PENDIENTE |
| `min_note_chars` | diario | 40 | Alcanza para «por qué entré y por qué salí» en una línea; frena notas vacías. | PENDIENTE |
| `note_deadline` | diario | corte del día de la salida | Lo que dice el texto de hoy en la UI. | PENDIENTE |
| `min_active_days` | límite | 8 días por mes | Unas 2 semanas de actividad: no se logra sin operar. | PENDIENTE |
| `proposal_ttl_minutes` | propuestas | 15 | Una vela de 15m: después de la apertura de `t+1` la propuesta ya no vale. | PENDIENTE |
| `min_review_seconds` | propuestas | 10 | Menos que eso es aprobar sin leer. | PENDIENTE |
| `window_minutes` | calendario | el de `config/news_filter.toml` (15) | Un solo número para el filtro y el trofeo. | PENDIENTE |
| `count_holding_through_event` | calendario | no | La spec §7.2 habla de abrir, no de mantener. | PENDIENTE |
| `min_entries_week` | calendario | 2 tickets manuales | Las del runner ya pasan por el filtro. | PENDIENTE |
| `min_manual_tickets` | tickets | 3 por semana | Con 1 o 2 cumplir es casi seguro. | PENDIENTE |
| `person_rejections` | tickets | la lista de §4.2 | Solo errores de armado del ticket; los de datos no cuentan. | PENDIENTE |
| `min_prereg_lead_hours` | preregistro | 12 | Obliga a escribir el criterio con tiempo, no un minuto antes. | PENDIENTE |
| `streak_weeks` | racha | 4 | Un mes de proceso cumplido, alcanzable y no trivial. | PENDIENTE |
| ¿Mostrar los que garantiza el sistema? | stop, calendario | sí, como «lo garantiza el sistema» | Es honesto y explica por qué no hay mérito. | PENDIENTE |
| ¿Sumar los 5 nuevos? | pausa, tickets, preregistro, cambios, racha | sí, de a uno | Cada uno usa datos que ya existen o casi. | PENDIENTE |

## 6. Dónde viviría el cálculo (cuando se apruebe)

- **Motor (Rust).** Un módulo nuevo `trophies` dentro de `cfds-journal`
  (`engine/crates/journal`), que ya junta propuestas, resultados, votos y walk-forward. Funciones
  puras: misma entrada (registros hasta el cierre del período + config), misma salida. Tests con
  casos borde: período sin actividad, calendario caído, período en curso, sin repintado.
- **Config:** `config/trophies.toml`, una sección por trofeo (`[trophies.common]`,
  `[trophies.stop]`, `[trophies.diario]`, `[trophies.limite]`, `[trophies.propuestas]`,
  `[trophies.calendario]`, y las de los nuevos), validada al arrancar. La ventana del calendario no
  se copia: se lee de `config/news_filter.toml`.
- **Contrato:** `contracts/trophy_status.v1.schema.json` (id del trofeo, período, estado, motivo en
  texto y las cantidades que se contaron, nunca montos de dinero), servido por el gateway en
  `GET /api/v1/trophies`. Los contratos futuros que hacen falta para algunos trofeos:
  `journal_note.v1`, `proposal_decision.v1` y `position_stop_change.v1`.
- **UI:** `ui/src/lib/trophy-rules.ts` deja de ser la fuente de las reglas: muestra el texto y el
  estado que manda el motor.
- **Orden sugerido:** primero los que ya tienen datos (límite, calendario, tickets, pausa), después
  los que necesitan un contrato nuevo (diario, propuestas, stop). El runner de paper arranca el
  27-11: los primeros datos reales llegan desde ahí.

## 7. Riesgos

- **Premiar no operar:** se frena con la actividad mínima y el estado «sin actividad».
- **Premiar resultados por la puerta de atrás:** ningún trofeo lee montos; «Dentro del límite» mira
  solo si el límite se tocó.
- **Trofeos sin mérito:** los que el sistema ya garantiza se dicen como tales.
- **Lenguaje de casino:** sin confeti ni puntos canjeables; acento sobrio
  (`docs/design/2026-11-rediseno-ui.md`).
- **Umbrales elegidos mirando los datos:** se fijan antes de que el runner tenga historia y no se
  ajustan para «dar» trofeos; un cambio de umbral pasa por propuesta y votación como cualquier
  parámetro.
