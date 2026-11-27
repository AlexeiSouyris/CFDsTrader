# Pre-registro del programa Nasdaq: H1 (deriva antes del comunicado del FOMC) y H2 (días de CPI y empleo) en el CFD US100

Issue #526 · 24-11-2026 · base: `docs/analysis/2026-11-programa-nasdaq-propuesta.md` (#527, §3) · método: el de las rondas 1 a 3 (`2026-11-ronda1-preregistro.md`, `2026-11-ronda2-preregistro.md`, `2026-11-ronda3-preregistro.md`).

Este archivo se commitea **antes de correr nada contra los precios**. La hora del commit que lo agrega es la del pre-registro, y su SHA-256 queda en `2026-11-programa-nasdaq-preregistro.md.sha256` y en la descripción del PR. Cambiar algo de acá después de correr invalida la hipótesis y cuenta como una prueba nueva.

Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real (`CLAUDE.md` §5).

## 0. Aprobación y presupuesto

- **Aprobación del dueño** (Alexei), 24-11-2026, en el chat con su agente, registrada en #526, textual: "apruebo H1 y H2, usá el 2,5 % del programa 2".
- **Presupuesto de error:** el 2,5 % que le quedaba de reserva al programa 2 (`2026-11-programa-2-propuesta.md` §3.1), repartido en **1,25 % unilateral para H1** y **1,25 % unilateral para H2** (Bonferroni entre 2). Con esto **el programa 2 queda en 0 %**: H1 y H2 son su familia E (eventos programados), aplicada solo al US100, y se corren **una sola vez** (propuesta §6, decisión 2, opción a).
- **H3 y H4** de la propuesta quedan en reserva **sin presupuesto**. No se pagan datos.
- **Cantidad de pruebas para el Sharpe deflactado:** **N = 356** = las 354 registradas hasta la ronda 3 (`config/research.toml`, `[rule_round3]`) más H1 y H2. Se guarda en `config/research.toml`, sección nueva `[rule_nasdaq]`, en el mismo PR que este archivo. Referencia también con N = 100 y N = 1000 (sin voto).
- Para preparar este archivo solo se leyó de la base, en modo solo lectura, la **cantidad de filas** de `official_releases` por estado, año y regla de hora (118 programadas de 2011 a 2025, 101 con el comunicado dentro de la validación; 4 no programadas; ninguna cancelada ni reprogramada). **No se leyó ningún precio.**

## 1. Lo que viene de las rondas anteriores sin cambios

- **Período:** la concatenación de las **51 ventanas de validación** del walk-forward de `config/data_split.toml` (24 meses de entrenamiento, 3 de validación, paso 3): **01-02-2013 → 01-11-2025** (153 meses). Las reglas de H1 y H2 **no tienen ningún parámetro que se estime**, así que la parte de entrenamiento de cada ventana no se usa (ver §2.6 sobre cómo se recorren las ventanas).
- **El fuera de muestra (desde el 01-11-2025) no se carga, no se mira y no se usa.** Las velas se piden solo hasta su inicio (función `load` de las rondas). La reunión del FOMC del 10-12-2025 queda fuera. Usarlo pide la aprobación del dueño, una sola vez, para un solo candidato congelado.
- **Velas:** futuros NQ (US100) de 1 minuto de Databento armados en velas de 15 m, `--fuente futuros`, con la preparación común (`examples/common/prepare.rs`: validación, horario de la spec, vencimientos encadenados, base del futuro de #470). Conexión con `default_transaction_read_only=on`.
- **Costos** (las mismas funciones del backtest de la estrategia en uso, `engine/crates/backtest/src/costs.rs`; los mismos supuestos que las rondas):
  - spread **1,6 puntos** en la sesión de contado (09:30–16:00 ET; `config/instruments.toml`, regla del mayor de #67);
  - spread **2,5 puntos** fuera de la sesión de contado (`docs/research/ig-spreads-2026-11.md`, franjas 00–13 y 23 UTC); solo lo usa la entrada de H2 (§4.2);
  - slippage por ejecución = 0,1 × spread + 1 tick (0,1 punto) (`config/backtest.toml`, `fill_price`);
  - comisión 0 (índices de IG);
  - financiación de IG **noche por noche** (tasa de referencia + 3 % anual sobre el nominal, base 360, tres noches el fin de semana), **menos la base del futuro** (#470, ADR 0015), con los **dos escenarios** empalmados de #421: **EUR** (`2026-11-eur-eonia-estr.csv`) y **USD** (`2026-11-usd-effr-sofr.csv`), porque IG no confirmó la tasa de referencia del US100;
  - **estrés:** todos los costos × 1,5 (criterio 4); × 2 solo se informa.
- **Tamaño:** exposición **1** = unidades = 1 × valor de la cuenta / precio medio de ejecución, igual nominal que comprar y mantener. Solo largos. Sin apalancamiento adicional.
- **Vara de comparación (x):** **comprar y mantener el CFD US100** con los mismos costos y los dos escenarios (`buy_and_hold_returns`, una cuenta nueva por ventana de validación), igual que R018 y los chequeos de #512.
- Retornos mensuales en UTC. Estadística en `f64`, dinero en `Decimal`.
- **Sin stop y sin `cfds_risk::check`:** como comprar y mantener y la familia V, H1 y H2 son pruebas de **exposición** a una ventana de tiempo, no estrategias para operar. Si alguna pasara, operarla pide un cambio de spec aprobado aparte (otros horarios, mantener una noche, stop); la prueba no lo habilita.

## 2. Reglas comunes de tiempo y de publicación

### 2.1 Zona horaria y horario de verano

Todas las horas "ET" son **hora local de Nueva York (`America/New_York`)**, con su horario de verano según la base de datos de zonas horarias (EST = UTC−5, EDT = UTC−4). Cada hora local se convierte a UTC **en la fecha de ese día** (por ejemplo, 14:00 ET es 19:00 UTC en enero y 18:00 UTC en julio). Las velas de 15 m se identifican por su **hora de inicio**.

### 2.2 Decisión y ejecución (sin mirar el futuro)

- "Entrar (o salir) en la apertura de la vela de las HH:MM ET" quiere decir: la **decisión** se toma al **cierre** de la vela anterior (la que empieza a HH:MM − 15 min), y la orden se ejecuta al precio de **apertura** de la vela que empieza a HH:MM (`t + 1`, `CLAUDE.md` §5b).
- Las dos velas (la de la decisión y la de la ejecución) tienen que existir y ser **consecutivas sin hueco** (la de la decisión termina justo cuando empieza la de la ejecución). Si falta alguna, ese evento **no se opera** y se informa en la lista de eventos no operados. No se busca otra vela.
- Un evento solo se usa si su fila del calendario ya existía en la decisión: **`published_at` ≤ cierre de la vela de la decisión** (`CLAUDE.md` §5b, ADR 0016 §4).

### 2.3 Calendario oficial: qué fila vale

Tabla `official_releases` (contrato `official_release.v1`, ADR 0016, #520/#521). Para cada evento (misma `source`, `series` y `reference_period`), en el momento de la decisión vale **la última fila con `published_at` ≤ decisión**:

- `scheduled` o `rescheduled`: el evento se opera con su `scheduled_at`;
- `cancelled`: el evento **no** se opera;
- si ninguna fila cumple `published_at` ≤ decisión: no se opera.

Con los datos de hoy no hay filas canceladas ni reprogramadas del FOMC; la regla queda fijada igual por si aparecen (y para H2, donde sí las hay).

### 2.4 FOMC no programados (decisión fijada ahora)

**Los comunicados no programados (`status = 'unscheduled'`) se ignoran por completo en H1:**

- **No se operan:** no se sabían antes (`published_at` = `scheduled_at`, ADR 0016 §4); una deriva **previa** a un anuncio que nadie esperaba no se puede medir sin mirar el futuro.
- **No anulan ni recortan una reunión programada:** si un comunicado no programado cae dentro de la ventana de una reunión programada, la operación de la programada se mantiene igual. Sacarla por algo que se supo **después** de la entrada sería mirar el futuro.
- La reunión programada del 17–18-03-2020, que la Fed reemplazó con la del domingo 15-03-2020 (no programada), **no figura** en `official_releases` como programada; por la regla 2.3 no se opera. Como el reemplazo se supo el 15-03, antes de la entrada que hubiera tenido (17-03, 14:00 ET), esto **no** mira el futuro.

### 2.5 Días hábiles

"Día hábil" = día con sesión de contado de XNYS según `config/sessions.toml` y el calendario de la preparación común (feriados y cierres tempranos incluidos).

### 2.6 Cómo se recorre el período

Una sola corrida continua de `managed_returns` (la misma función de la familia V y de comprar y mantener) sobre **[01-02-2013, 01-11-2025)**, con peso 1 durante cada ventana de evento y 0 fuera de ellas. Como la regla no tiene parámetros que se entrenen y la exposición es proporcional al valor de la cuenta, el retorno de cada mes es el mismo que con una cuenta nueva por ventana de validación; la diferencia es que un evento que cruza el borde entre dos ventanas (por ejemplo, entrada el 30-04 y comunicado el 01-05) **se opera** en vez de perderse. La corrida tiene que dar exactamente los **153 meses** de las ventanas; si da otros, se detiene y se informa.

Los meses sin evento dan retorno 0 (sin posición no hay costos ni financiación). Entran igual en todas las estadísticas.

## 3. H1. Deriva antes del comunicado del FOMC (Lucca y Moench, 2015)

### 3.1 Eventos

Filas de `official_releases` con `source = 'fed_fomc'`, `series = 'fomc_statement'` y estado según §2.3 (programadas o reprogramadas; nunca no programadas, §2.4), cuya vela de **salida** empieza dentro del período [01-02-2013, 01-11-2025). Esperado: **101 reuniones programadas** (el número final sale de la lista de eventos operados y no operados, que se informa entera).

### 3.2 Regla (fija, sin parámetros ajustables)

- **Día del comunicado** `D` = la fecha en ET de `scheduled_at`.
- **Entrada:** largo, exposición 1 (§1), en la **apertura de la vela de las 14:00 ET del día hábil anterior a `D`** (§2.5; el día hábil anterior puede ser el viernes si `D` es lunes). Decisión al cierre de la vela de las 13:45 ET (§2.2). Si ese día hábil anterior cierra temprano y no tiene vela de las 14:00 ET dentro de la sesión, el evento no se opera.
- **Salida:** cierre total en la **apertura de la vela de 15 m que empieza 30 minutos antes de `scheduled_at`** (redondeado hacia abajo a un múltiplo de 15 minutos), el mismo día `D`. Por ejemplo: comunicado de las 14:00 ET → salida en la apertura de la vela de las 13:30 ET; de las 14:15 ET → 13:45 ET; de las 12:30 ET → 12:00 ET. Decisión al cierre de la vela anterior (§2.2). El margen de 30 minutos cubre las horas aproximadas anteriores a 2020 (ADR 0016 trata la hora como ± 15 minutos).
- **Publicación:** la fila tiene que cumplir `published_at` ≤ cierre de la vela de las 13:45 ET del día de la entrada (§2.2 y §2.3).
- Una posición a la vez. Si dos ventanas de evento se superpusieran, la segunda no se opera (con el calendario del FOMC no pasa).
- **Spread:** la entrada y la salida caen dentro de la sesión de contado: **1,6 puntos** en las dos. Financiación: las noches que cruza la ventana (una, o tres si cruza un fin de semana).
- **No se miran** la conferencia de prensa, el resultado de la reunión, el precio ni la volatilidad para decidir.

### 3.3 Series

- `y` = retornos mensuales netos de H1 (153 meses), cada escenario de financiación.
- `x` = retornos mensuales de comprar y mantener US100 (§1), mismo escenario.
- `y_s`, `x_s` = las dos series con todos los costos × 1,5.

### 3.4 Criterios (todos, con los dos escenarios EUR y USD)

1. **Alfa contra comprar y mantener:** regresión por mínimos cuadrados `y = α + β x + e`, error estándar de `α` robusto (White, HC1: factor `n / (n − 2)`), prueba t unilateral (H0: α ≤ 0) con `n − 2` grados de libertad (`research::stats::alpha_hc1`). Pasa si **p < 0,0125**.
2. **Sharpe deflactado de la ventaja:** la serie `y_t − β x_t` (alfa + residuo, con el `β` del criterio 1), con la fórmula de la ronda 1 §2.5 (`research::stats::deflated_sharpe`) y **N = 356**. Pasa si **DSR ≥ 0,95**. Si la fórmula no da valor (por ejemplo, desvío cero o denominador ≤ 0), no pasa.
3. **Consistencia por mitades:** la media de `y_t − β x_t` (con el `β` de toda la muestra) es **> 0** en **02-2013 → 12-2020** y también en **01-2021 → 10-2025** (segunda mitad desde el 01-01-2021, como en la propuesta §3.2 y los chequeos de #512).
4. **Costos × 1,5:** el alfa de la misma regresión con `y_s` y `x_s` sigue **> 0**.
5. **Agrega a comprar y mantener:** la cartera mensual `0,5 x_t + 0,5 y_t` (rebalanceada cada mes, sin costo extra de rebalanceo) tiene un **Sharpe mensual mayor** que el de `x` solo.

**H1 pasa solo si cumple los 5 criterios en los dos escenarios.** Si falla uno solo en un escenario, no pasa.

### 3.5 Descriptivo (sin voto, sin presupuesto)

- Lista completa de eventos: fecha, hora del comunicado, regla de hora, con o sin conferencia, operado o no (y por qué).
- Por evento: retorno de la ventana **antes de costos** y **neto** (pb), media, desvío, t y cantidad; lo mismo por mitades y por "con / sin conferencia de prensa".
- Costo medio por evento (spread + slippage + financiación − base), en pb.
- La misma regla, idéntica, en **US500 y US30** (futuros ES e YM): alfa y p contra su propio comprar y mantener. Solo como contexto: no vota.
- Retorno mensual medio, Sharpe y meses con posición de H1; los mismos de comprar y mantener; DSR con N = 100 y N = 1000; resultado con costos × 2.

## 4. H2. Días de CPI y empleo (Savor y Wilson, 2013) — congelada, se corre cuando esté el BLS

### 4.1 Eventos

Filas de `official_releases` con `source = 'bls'` y `series` en (`cpi`, `employment_situation`), con la regla de §2.3 (vale la última fila publicada antes de la decisión; `cancelled` no se opera). **Hoy no están cargadas** (ADR 0016, opción A elegida por el dueño el 24-11-2026, #505). H2 **no se corre** hasta que la carga del BLS esté mergeada en `main` con sus propias verificaciones. Esta definición queda congelada desde ahora; no se cambia al ver los datos del BLS.

### 4.2 Regla (fija, sin parámetros ajustables)

- **Día del dato** `D` = la fecha en ET de `scheduled_at` de la fila vigente.
- Solo días con sesión de contado de XNYS (§2.5). Si el mercado de contado no abre ese día (por ejemplo, un empleo publicado en Viernes Santo), no se opera.
- Solo datos de las **08:30 ET**. Si `scheduled_at` tiene otra hora, el evento no se opera y se informa.
- **Entrada:** largo, exposición 1, en la **apertura de la vela de las 08:00 ET** de `D` (decisión al cierre de la vela de las 07:45 ET). Requiere `published_at` ≤ 08:00 ET de `D`.
- **Salida:** cierre total en la **apertura de la última vela de 15 m de la sesión de contado de `D`** (15:45 ET un día normal; 12:45 ET un día de cierre temprano). Decisión al cierre de la vela anterior.
- Si el mismo día hay CPI y empleo, se opera **una sola vez**.
- **Spread:** la entrada (08:00 ET) es **fuera de la sesión de contado → 2,5 puntos**; la salida, **dentro → 1,6 puntos**. Slippage = 0,1 × el spread de esa ejecución + 1 tick. Sin noches: sin financiación.
- Todo lo de §2 (velas consecutivas, sin hueco, publicación) vale igual.

### 4.3 Criterios

Los mismos 5 de §3.4, con `y` = los retornos mensuales de H2, la misma `x`, **p < 0,0125**, **N = 356** (el N al congelar; con el N vigente al correr se informa además, sin voto), las mismas mitades y los dos escenarios. Mismo descriptivo que §3.5 (cambiando "con / sin conferencia" por "CPI / empleo / los dos el mismo día").

### 4.4 Aviso de poder

Con el efecto supuesto en la propuesta (§2), H2 tiene ≈ 26 % de probabilidad de pasar aunque el efecto exista. Se dice antes para no sorprenderse después.

## 5. Qué pasa con el resultado

- **Pasa** → se informa al dueño. Él decide si se mira el fuera de muestra (una sola vez, con la regla congelada tal cual) y si se abre un Issue de spec para operarla. Pasar **no** habilita operar.
- **No pasa** → la hipótesis se cierra. **No se prueban variantes** sobre estos datos.
- Cada corrida agrega su fila en `docs/analysis/registro-de-resultados.csv` en el mismo PR que su informe (H1: `docs/analysis/2026-11-programa-nasdaq-h1-resultados.md`).

## 6. Prohibido después de ver los resultados

Todo esto cuenta como una prueba nueva, con su costo en el presupuesto (y hoy no queda presupuesto):

- cambiar la hora de entrada (14:00 ET del día anterior; 08:00 ET), la de salida (30 minutos antes; última vela de la sesión) o el tamaño;
- agregar o sacar eventos a mano, filtrar por conferencia de prensa, por año, por resultado de la reunión, por volatilidad o por cualquier otra cosa (los cortes del §3.5 son descriptivos y no votan);
- tratar de otro modo los FOMC no programados, las cancelaciones o las reprogramaciones (§2.3 y §2.4);
- cambiar los costos, el escenario de financiación (los dos votan; no se elige uno), el estrés, la vara de comparación, el corte de las mitades, el nivel 0,0125, el mínimo 0,95 o **N = 356**;
- cambiar el período o mirar el fuera de muestra sin la aprobación del dueño;
- pasar a otro índice como "el principal" porque dio mejor (US500 y US30 son descriptivos);
- volver a correr con otra regla y presentar la mejor.

**Errores de código:** si después de ver los resultados se encuentra un error de código (no de la regla), se corrige, se vuelve a correr y se informa **con fecha**: cuál fue el error, el resultado anterior y el nuevo. El resultado anterior **no se borra** (en el informe y como fila propia en el registro, con `compara_con`). La regla de este archivo no cambia.

## 7. Parámetros en config

- Fase 1 (este PR): `config/research.toml`, sección nueva **`[rule_nasdaq]`**: `alpha = "0.0125"`, `dsr_trials = 356`, `dsr_min = "0.95"`, `cost_stress = "1.5"`, `second_half_from = "2021-01-01"`. Validada al cargar.
- Fase 2 (PR de H1): las horas de esta regla (14:00 ET, 30 minutos, zona `America/New_York`; y las de H2: 08:00 ET, spread fuera de sesión 2,5 puntos) van en secciones nuevas de `config/research.toml`, **con los valores exactos de este archivo**. Las secciones de las rondas 1 a 3 no cambian. `config/strategy.toml`, `config/risk.toml`, el runner y el paper no cambian.

## 8. Registro de corridas

Cada corrida (aunque falle o se repita por un error de código) se anota en el informe con su hora (UTC) y su comando. Antes de calcular retornos, la corrida imprime la **lista de eventos operables y no operables** (eso no mira precios más allá de que la vela exista).

## 9. Referencias

- Lucca, D. O. y Moench, E. (2015). *The pre-FOMC announcement drift*. Journal of Finance, 70(1), 329–371.
- Savor, P. y Wilson, M. (2013). *How much do investors care about macroeconomic risk? Evidence from scheduled economic announcements*. Journal of Financial and Quantitative Analysis, 48(2), 343–375.
- Bailey, D. H. y López de Prado, M. (2014). *The deflated Sharpe ratio*. Journal of Portfolio Management, 40(5), 94–107.
- ADR 0016 (calendario histórico oficial), ADR 0015 (base del futuro), `docs/research/ig-spreads-2026-11.md`.
