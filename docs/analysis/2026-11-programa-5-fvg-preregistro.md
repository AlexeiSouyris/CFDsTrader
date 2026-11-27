# Programa 5: pre-registro. Reacción al primer toque de un FVG (US100 y US500)

> **Estado: CONGELADO el 25-11-2026** (Issue #649). Escrito por un agente de IA (Claude Code). El dueño aprobó los 20 puntos abiertos de §11 con los valores recomendados el 25-11-2026: «apruebo los 20....» (https://github.com/AlexeiSouyris/CFDsTrader/issues/649#issuecomment-7164689340). Sus decisiones, el conteo previo de H5.2 (punto 16) y las precisiones de implementación están en §13. La huella SHA-256 de este archivo está en `2026-11-programa-5-fvg-preregistro.md.sha256`: si el archivo cambia, la huella deja de coincidir.
>
> Antes de congelar solo se **contaron eventos y operaciones** (§7 y §13.2). **No se calculó ninguna reacción, ningún retorno, ningún P/L ni ningún resultado.** Cambiar algo de acá después de correr invalida la hipótesis y cuenta como una prueba nueva.
>
> Método y formato: el pre-registro del programa 4 (`2026-11-programa-4-preregistro.md`), el del programa Nasdaq (`2026-11-programa-nasdaq-preregistro.md`) y el control de M1 de la ronda 3 (`2026-11-ronda3-preregistro.md` §4). Definiciones de FVG: las del motor (`engine/crates/liquidity/src/fvg.rs` y `stats.rs`, spec §5.4 y §5.4.1).

Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real (`CLAUDE.md` §5): aunque una hipótesis pase, queda en simulación. **No usa nada de Gianluca Bruni** (eso es el programa 4).

## Resumen simple

**Qué es un FVG.** Un *fair value gap* (hueco de valor) aparece cuando el precio se mueve tan rápido que entre la vela de hace dos y la actual queda un tramo de precio por el que casi no se operó. Muchos traders creen que, cuando el precio **vuelve por primera vez** a ese hueco, "rebota" a favor del movimiento que lo creó. El motor ya detecta los FVG y ya mide ese rebote (en velas de 15 minutos y de 1 hora), pero hasta ahora **nadie comprobó si el rebote es real** o si es lo que pasaría tocando cualquier precio.

**Qué se quiere saber (dos preguntas):**

1. **H5.1 (¿el FVG importa?).** Cuando el precio toca un FVG por primera vez, ¿rebota más que cuando toca **un precio cualquiera** elegido al azar en las mismas condiciones (mismo mes, mismo horario, misma distancia recorrida, mismo tamaño)? Ese precio al azar es el *placebo*: si el FVG no rebota más que el placebo, el FVG no agrega nada.
2. **H5.2 (¿sirve como filtro?).** De las operaciones que propone la estrategia actual, ¿las que coinciden con el primer toque de un FVG a favor ganan más que las otras?

**Cómo se evita engañarse:** las reglas se escriben **antes** de mirar resultados; se usan los años 2013 a octubre de 2025 de los futuros del Nasdaq 100 y del S&P 500; **desde noviembre de 2025 no se mira nada** (es la prueba final); tiene que funcionar en las dos mitades del período y en los dos índices; los costos se cargan y además se prueban un 50 % más caros; y se compara contra el azar.

**Cuántos casos hay** (§7 y §13.2): unos **40.000 primeros toques** por índice en velas de 15 minutos y unos **10.000** en velas de 1 hora. Sobra muestra para H5.1. Para H5.2, de unas 2.600 operaciones de la estrategia por índice, **577 (US100) y 565–566 (US500)** coinciden con un primer toque: más que el mínimo de 100.

**Estado:** el dueño aprobó los **20 puntos** de §11 con los valores recomendados (§13). El pre-registro está congelado y se puede correr.

## 0. Aprobación y presupuesto

- **Pedido del dueño** (Alexei), 25-11-2026, textual: «si armalo», sobre la propuesta de H5.1 y H5.2 (registrado en #649).
- **Aprobación del dueño** (Alexei), 25-11-2026, textual: «apruebo los 20....», sobre los 20 puntos abiertos de §11 con los valores recomendados (registrado en #649). Aprueba el presupuesto y todos los parámetros (§13).
- **Presupuesto propuesto** (punto abierto 1): un **5 % nuevo** de falsos positivos para el programa 5, como los programas 2 y 4, repartido por Bonferroni, unilateral:

| Hipótesis | Qué prueba | Nivel propuesto |
|---|---|---|
| H5.1 | Reacción al primer toque de un FVG frente a un placebo emparejado | 2,5 % (p < 0,025) |
| H5.2 | Propuestas de la estrategia en uso que coinciden con un FVG a favor frente al resto | 2,5 % (p < 0,025) |

- Sin reserva: son solo dos hipótesis y la muestra es grande; partir el 5 % en más pedazos solo quita poder.
- **Ojo con el total del proyecto:** cada programa con su 5 % propio sube la probabilidad de que algún resultado del proyecto sea un falso positivo. Es lo mismo que se decidió para los programas 3 y 4 (§13, decisión 1 del programa 4).
- **Cantidad de pruebas para el Sharpe deflactado:** el N del registro **al congelar**. Hoy es **359** (`config/research.toml`, `[rule_program4]`). Al congelar se suman H5.1 y H5.2: **N = 361**, salvo que antes se congele otro programa (se recuenta al congelar). Referencia con N = 100 y N = 1000, sin voto.

## 1. Lo que viene de los programas anteriores sin cambios

- **Período:** la concatenación de las **51 ventanas de validación** del walk-forward de `config/data_split.toml` (24 meses de entrenamiento, 3 de validación, paso 3): **01-02-2013 → 01-11-2025** (153 meses). Ni H5.1 ni H5.2 tienen parámetros que se estimen con los datos (todos se fijan antes, §11), así que la parte de entrenamiento no se usa, salvo para el calentamiento (ATR, FVG creados antes del 01-02-2013 cuyo primer toque cae dentro del período).
- **El fuera de muestra (desde el 01-11-2025) no se carga, no se mira y no se usa.** Un evento cuenta solo si su toque **y** las 8 velas de su reacción empiezan antes del 01-11-2025. Usar el fuera de muestra pide la aprobación del dueño, una sola vez, para un solo candidato congelado.
- **Costos** (`engine/crates/backtest/src/costs.rs`, los mismos supuestos que el programa 4 §1): spread de `config/instruments.toml` en la sesión de contado (US100 1,6 puntos; US500 0,9 puntos) y el de `docs/research/ig-spreads-2026-11.md` fuera de ella; slippage = 0,1 × spread + 1 tick; comisión 0; financiación de IG noche por noche menos la base del futuro (#470, ADR 0015) con los **dos escenarios** EUR y USD de #421. **Estrés:** todos los costos × 1,5 (vota en H5.2); × 2 solo se informa.
- **Mitades:** **02-2013 → 12-2020** y **01-2021 → 10-2025** (`second_half_from = "2021-01-01"`).
- Estadística en `f64`, dinero en `Decimal`, horas en UTC.

## 2. Datos

- **Precio:** velas de **15 minutos y de 1 hora** de los futuros de Databento (`futures_candles`, `source = 'databento'`; NQ para US100 y ES para US500), armadas desde `futures_bars_1m` (#259), con su `contract_id`. Conexión de solo lectura. Las velas se identifican por su hora de inicio.
- **Se recalcula con las funciones del motor**, no se leen las tablas guardadas: hoy `fvg_zones` y `fvg_zone_events` de `US100.FUT` y `US500.FUT` empiezan en **11-2021** (no cubren el período) y llegan a 11-2026 (**incluyen el fuera de muestra**). La corrida llama a `cfds_liquidity::stats::fvg_events` y `fvg_created_at` sobre las velas hasta el 01-11-2025.
- **CFD contra futuro:** el FVG, el toque y la reacción en ATR son **diferencias de precio dentro de un mismo contrato**; la base del futuro casi no cambia en 8 velas, así que en ATR son casi iguales a las del CFD. Lo que no es igual es el costo, y por eso H5.2 usa los costos del CFD (§1).
- **IBKR (MNQ y MES, `US100.IBKR` y `US500.IBKR`):** sus velas empiezan el **14-09-2026**, es decir, **todas caen dentro del fuera de muestra**. Usarlas, aunque sea como descriptivo, sería mirar el período de la prueba final. Se propone **no usarlas en este programa** (punto abierto 17).

## 3. Definiciones (las del motor, sin cambios)

Para el lado alcista; el bajista es el espejo exacto. `H`, `L`, `C` = máximo, mínimo y cierre de cada vela; los índices cuentan las velas **presentes** (las faltantes no se rellenan), igual que en el motor.

- **FVG alcista creado en la vela `i`** (`fvg_created_at`): `L[i] > H[i−2]`; zona **[piso, techo] = [H[i−2], L[i]]**. Bajista: `H[i] < L[i−2]`, zona [H[i], L[i−2]]. Existe desde el **cierre** de `i`.
- **ATR:** el de Wilder de 14 velas del mismo timeframe (`wilder_atr`, con rango verdadero; `[order_blocks] atr_period = 14`). Sin ATR válido (> 0) en `i` no hay FVG (calentamiento).
- **Tamaño** = (techo − piso) / ATR[i].
- **Llenado** (`is_filled_by`): la primera vela posterior a `i` que **cierra** por debajo del piso. Desde ahí el FVG deja de valer.
- **Primer toque** (`fvg_events`): la primera vela `t > i` con `L[t] ≤ techo`, mientras el FVG no se llenó y `t ≤ i + 500` (`[fvg_stats] max_age_bars`). Si la vela del toque también cierra por debajo del piso, el motor la cuenta como toque **y** llenado.
- **Reacción a `k` velas** (`[fvg_stats] reaction_bars = [4, 8]`): `r_k = (C[t + k] − C[t]) / ATR[i]`, con signo a favor del FVG (en el bajista se cambia el signo). Se conoce al cierre de `t + k`. La decisión "toco el FVG" se toma al **cierre** de `t`, así que medir desde `C[t]` no mira el futuro.

**Filtros de este programa** (no cambian la definición del motor; solo eligen qué eventos entran):

- **F1. Sin vencimientos de por medio:** las velas `i − 2`, `i − 1`, `i`, la del toque `t` y las de `t + 1` a `t + 8` son del **mismo contrato** (`contract_id`). Así el salto entre contratos no crea un FVG falso ni una reacción falsa (punto abierto 7).
- **F2. El toque no llena:** se excluye el toque que en la misma vela cierra por debajo del piso (C[t] < piso). Es lo que el contexto de la spec ya exige para "tocar" (`fvg_touches_at`: entra a la zona sin llenarla) y se sabe al cierre de `t` (punto abierto 4).
- **F3. Dentro del período:** `t` empieza en o después del 01-02-2013 y `t + 8` empieza antes del 01-11-2025.
- **F4. Tamaño mínimo:** ninguno (el motor no tiene; punto abierto 3).
- **F5. Espera:** el motor permite el toque desde `t = i + 1` (punto abierto 5).

A un evento que pasa F1 a F5 se lo llama **evento real**.

## 4. H5.1 La reacción al primer toque frente a un placebo emparejado

**Hipótesis:** en velas de 15 m de US100 y US500, la reacción a **8 velas** después del primer toque de un FVG (a favor del FVG, en ATR) es **mayor** que la de un "toque" de un precio elegido al azar con las mismas condiciones.

Es una prueba de **contexto**: no opera, no tiene entrada, stop ni objetivo. Si pasa, recién ahí tiene sentido pensar en usar el FVG para operar (H5.2 o un programa futuro).

### 4.1 El placebo (punto abierto 9)

Para cada evento real `e` (instrumento, timeframe, dirección, toque en `t`, creado en `i`) se guardan:

- su **distancia de llegada** `d(e)` = (C[t − 1] − techo) / ATR[i] (alcista; bajista: (piso − C[t − 1]) / ATR[i]). Siempre es ≥ 0, porque antes del primer toque ninguna vela entró a la zona;
- su **tamaño** `w(e)` = tamaño del FVG en ATR;
- su **espera** `a(e)` = t − i;
- su **franja**: sesión de contado de XNYS (la vela del toque termina entre las 09:30 y las 16:00 ET, `config/sessions.toml`) o fuera de ella.

**Velas candidatas a placebo de `e`:** cada vela `s` del mismo instrumento y timeframe que cumple todo esto:

1. empieza en el **mismo mes calendario (UTC)** que `t` y en la **misma franja**;
2. con el nivel `P = C[s − 1] − d(e) × ATR[s − 1]` y el piso falso `B = P − w(e) × ATR[s − 1]` (alcista; bajista: espejo, `P = C[s − 1] + d(e) × ATR[s − 1]`, `B = P + w(e) × ATR[s − 1]`):
   - **toca:** `L[s] ≤ P`;
   - **no "llena":** `C[s] ≥ B`;
   - **es un primer toque con la misma espera:** las velas `s − a(e) + 1` a `s − 1` tienen todas `L > P` (si `a(e) = 1`, no se exige nada más);
3. **no es un evento real** de la misma dirección (ninguna vela `s` que toca por primera vez un FVG real de esa dirección);
4. `s − a(e) − 2` a `s + 8` son del mismo contrato y hay ATR válido en `s − 1`; `s + 8` empieza antes del 01-11-2025.

Para cada evento real se sortean **5 placebos** con reposición entre sus candidatas (SplitMix64, §6). Si no tiene ninguna candidata, el evento sale del análisis (de los dos lados) y se cuenta aparte.

**Reacción del placebo:** `r_k(s) = (C[s + k] − C[s]) / ATR[s − 1]`, con el signo de la dirección del evento real.

**Por qué así:** el placebo copia todo lo que no es "ser un FVG": la época (mes), el horario, el lado y la distancia que el precio tuvo que recorrer para llegar, el ancho de la zona y que sea la primera vez que se llega después de la misma espera. Lo único que le falta es el hueco de la vela de impulso. Es la misma idea que el control de M1 (ronda 3 §4.1: el nivel redondo contra el nivel corrido medio paso), llevada a eventos.

**Nota sobre el ATR:** el evento real se mide con ATR[i] (el del motor) y el placebo con ATR[s − 1]. Como la vela `i` es una de impulso, ATR[i] tiende a ser algo mayor que ATR[t − 1]; eso achica la reacción real en ATR y juega **en contra** de H5.1 (conservador). Como descriptivo se informa la reacción real medida con ATR[t − 1] (punto abierto 10).

### 4.2 Estadística

- Para cada instrumento y cada mes `m` de los 153: `D_m` = media de `r_8` de los eventos reales con toque en `m` − media de `r_8` de sus placebos.
- **Serie combinada:** `D̄_m` = promedio de `D_m` de US100 y de US500 (si un instrumento no tiene eventos en `m`, se usa el otro; si ninguno, el mes no entra).
- **Prueba:** t unilateral de la media de `D̄_m` (H0: media ≤ 0), con error estándar de Newey-West de 3 rezagos (función nueva en `research::stats`, con tests contra valores calculados a mano) y `n − 1` grados de libertad. Los eventos de un mismo mes no son independientes (se superponen y comparten régimen); trabajar con medias mensuales lo resuelve como en los programas anteriores (punto abierto 11).

### 4.3 Criterios para pasar (todos)

1. **Significativa:** p < **0,025** (nivel del punto abierto 1) en la prueba de §4.2.
2. **Los dos índices:** la media de `D_m` es **> 0** en US100 y en US500 por separado.
3. **Mitades:** la media de `D̄_m` es **> 0** en 02-2013 → 12-2020 y en 01-2021 → 10-2025.
4. **Otro horizonte:** la media de la serie combinada con `r_4` (4 velas) también es **> 0**.
5. **Muestra mínima:** al menos **1.000 eventos reales con placebo** por instrumento (punto abierto 15; con §7 se espera tener unos 40.000).

**Costos en H5.1** (punto abierto 12): H5.1 no opera, así que los costos **no votan**. Se informa, por instrumento y por mitad, la diferencia `real − placebo` en **puntos** (D en ATR × ATR en puntos de cada evento) junto al costo de ida y vuelta del CFD en esa franja, × 1 y × 1,5. Sirve para saber, si pasa, si el efecto alcanza a pagar los costos.

### 4.4 Descriptivo (sin voto)

- Eventos reales por instrumento, año, dirección y franja; cuántos se perdieron por F1, F2 y por no tener placebo.
- Media, mediana y rango intercuartílico de `r_4` y `r_8` reales y placebo; la diferencia por **tercil de tamaño** (los del motor), por dirección, por franja (contado / fuera), por espera (1, 2–5, 6–20, > 20 velas) y por año.
- Lo mismo en **velas de 1 hora** (punto abierto 2).
- Reacción real con ATR[t − 1] en vez de ATR[i] (§4.1).
- Proporción de eventos reales con `r_8 > 0` frente a la de los placebos, con IC de Wilson.
- Test de suerte (§6).

## 5. H5.2 Las propuestas de la estrategia en uso que coinciden con un FVG a favor

**Hipótesis:** entre las operaciones de la estrategia en uso (spec §6 a §8, `config/strategy.toml` y `config/risk.toml` **sin cambios**) en US100 y US500, las que coinciden con el primer toque de un FVG a favor tienen un **R neto medio mayor** que las demás.

**No hay reglas de entrada, stop ni salida nuevas:** se usan las operaciones que la estrategia ya genera, con su entrada en `t + 1`, su stop estructural, su TP1, su tamaño por riesgo y su gestión. Solo se les pone una etiqueta.

### 5.1 Base

- Las operaciones del ejemplo `diagnostico` (`engine/crates/backtest/examples/diagnostico.rs`, #323 y #456) con `--fuente futuros --tramos`, en los **bloques trimestrales de las 51 ventanas de validación** (02-2013 → 10-2025), con los dos escenarios de financiación. Cada bloque arranca con el mismo capital y el calentamiento de `config/evaluation.toml`, como en el diagnóstico de #456 (que tiene unas 3.000 operaciones por índice en 2011–2025).
- **R neto** = P/L de la operación después de todos los costos / riesgo inicial (como el diagnóstico).

### 5.2 Etiqueta "con FVG a favor" (punto abierto 13)

Una operación larga (corta) con decisión al cierre de la vela de 15 m `t` es **F** si `t` es la vela del toque de un **evento real alcista (bajista)** de §3 en velas de 15 m: un primer toque de un FVG de la misma dirección que pasa F1, F2 y F4. Se conoce al cierre de `t`: no mira el futuro. Las demás operaciones son **R** (resto).

**Aviso:** el puntaje de la estrategia ya suma +1 por "toque de FVG u order block" (spec §6, con la definición de contexto: cualquier toque de un FVG vigente de las últimas 50 velas). Muchas operaciones F tuvieron ese punto, y algunas existen solo por él. Se informa cuántas.

### 5.3 Criterios para pasar (todos, en los dos escenarios EUR y USD)

1. **Mejor que el resto:** la diferencia entre el R neto medio de F y el de R, juntando US100 y US500, es **> 0** con una prueba t de Welch unilateral con **p < 0,025**.
2. **Los dos índices:** la diferencia es **> 0** en US100 y en US500 por separado.
3. **Mitades:** la diferencia es **> 0** en 02-2013 → 12-2020 y en 01-2021 → 10-2025.
4. **Costos × 1,5:** con todos los costos × 1,5, la diferencia sigue **> 0**.
5. **Muestra mínima:** al menos **100 operaciones F** por instrumento (punto abierto 15).

**H5.2 pasa solo si cumple los 5 en los dos escenarios.** Pasar quiere decir "el FVG separa operaciones mejores de peores", **no** que la estrategia filtrada gane: la estrategia en uso pierde por costos (R001–R003), y un filtro puede mejorarla sin volverla positiva.

### 5.4 Estrategia filtrada (descriptivo, o vota según el punto abierto 14)

La estrategia en uso **solo con las operaciones F** (las R no se toman; se vuelve a simular con una posición a la vez, porque al no tomar una R puede aparecer una F que antes se perdía), con los criterios de siempre:

- alfa contra comprar y mantener el CFD (`alpha_hc1`), Sharpe deflactado de la ventaja con el N al congelar (`deflated_sharpe`), mitades, costos × 1,5 y la cartera 50/50 con comprar y mantener (los 5 criterios del programa 4 §7);
- se informan siempre; **votan solo si el dueño lo decide** en el punto abierto 14 (la recomendación es que no voten).

### 5.5 Descriptivo (sin voto)

- Operaciones F y R por instrumento, año, dirección y gatillo; cuántas F tuvieron el +1 de FVG u order block en el puntaje.
- R medio **antes de costos** y neto de F y de R, aciertos, salidas por motivo (stop, TP1, cierre forzado), con IC.
- La diferencia F − R por tercil de tamaño del FVG y con la variante "toque en `t` o en `t − 1`".
- Resultado con costos × 2.
- Test de suerte (§6).

## 6. Test de suerte (solo reporte) y generador

- **Generador:** **SplitMix64** (Steele, Lea y Flood, 2014), semilla **20261125**, un solo flujo en orden fijo (instrumento US100 y luego US500; eventos en orden de tiempo). Índice uniforme en [0, n) = (x · n) >> 64; número en [0, 1) = (x >> 11) / 2⁵³. Es el mismo del programa 4 §13.2 (punto abierto 18).
- **H5.1:** el sorteo de §4.1 se repite **1.000 veces** (a continuación en el mismo flujo). Se informa en qué percentil de esas 1.000 medias de placebo cae la media real de `r_8` (combinada y por instrumento).
- **H5.2:** **1.000 subconjuntos al azar** de las operaciones de la estrategia, del mismo tamaño que F en cada instrumento. Se informa en qué percentil cae el R neto medio de F.
- No vota: es una forma fácil de leer el resultado.

## 7. Conteo de eventos antes de congelar (sin resultados)

Corrido el **25-11-2026** por el agente con un script de un solo uso (Python, conexión con `default_transaction_read_only=on`) que **reproduce** las definiciones de §3 (creación, ATR de Wilder 14 sobre toda la serie, llenado, primer toque hasta 500 velas) sobre `futures_candles` **hasta el 01-11-2025** (el fuera de muestra no se cargó). **Solo cuenta**: no calcula reacciones, retornos ni nada del precio después del toque, salvo que las 8 velas existan. El conteo oficial se repite al correr con las funciones del motor (Rust); puede diferir en pocos casos.

Primeros toques con la vela del toque entre el 01-02-2013 y el 01-11-2025 y sus 8 velas siguientes antes del 01-11-2025:

| Instrumento | TF | Todos | F1 (sin vencimiento) | F1 + F2 (recomendado) | … + tamaño ≥ 0,25 ATR | … + toque en contado | … + espera ≥ 3 velas |
|---|---|---|---|---|---|---|---|
| US100 | 15m | 59.636 | 59.174 | **45.731** | 27.556 | 13.605 | 18.624 |
| US100 | 1h | 15.064 | 14.720 | **11.074** | 6.592 | 3.989 | 4.679 |
| US500 | 15m | 51.101 | 50.738 | **41.436** | 26.017 | 12.820 | 16.083 |
| US500 | 1h | 13.669 | 13.355 | **10.516** | 6.354 | 3.691 | 4.190 |

Otros cortes de F1 + F2 en 15m: tamaño ≥ 0,1 ATR 38.932 (US100) y 37.389 (US500); ≥ 0,5 ATR 15.273 y 13.676; tamaño ≥ 0,25 y toque en contado 8.910 y 8.589.

Por mitades (F1 + F2, 15m): US100 28.560 (2013-02 → 2020) y 17.171 (2021 → 2025-10); US500 24.614 y 16.822. Por año, entre 2.683 y 3.709 por índice en 15m (2025 hasta octubre: 2.892 y 2.956) y entre 709 y 895 en 1h. **Todos los años y las dos mitades superan con holgura el mínimo de 1.000** (punto abierto 15).

**IBKR** (solo de las tablas ya guardadas, sin recalcular): `US100.IBKR` 932 FVG tocados en 15m y 254 en 1h; `US500.IBKR` 909 y 243; todos desde el 14-09-2026, **dentro del fuera de muestra** (§2).

**H5.2:** contadas antes de congelar con el punto 16 aprobado (§13.2): **577 operaciones F en US100 y 565–566 en US500**, más que el mínimo de 100.

## 8. Fuera de alcance

- Operar el FVG solo (entrada en el toque, stop detrás de la zona, objetivo): sería un programa nuevo, solo si H5.1 pasa.
- Otras definiciones de FVG (por cuerpo, con tamaño mínimo distinto, "inversion FVG", "consequent encroachment" como nivel de entrada), otros timeframes (5 m, diario), otros instrumentos (US30, US2000, DE40, oro).
- Cambiar el puntaje de la estrategia (por ejemplo, separar FVG de order block): si H5.2 pasa, es un cambio de spec aparte, aprobado por el dueño.
- Todo lo de Gianluca Bruni (programa 4).
- **Pasar a real:** aunque algo pase, operarlo pide un cambio de spec aprobado aparte, y el dinero real necesita un ADR de dos personas (`CLAUDE.md` §5).

## 9. Prohibido después de ver resultados

Todo esto cuenta como una prueba nueva, con su costo en el presupuesto:

- cambiar la definición de FVG, de toque, de llenado, el tamaño mínimo, la espera, los 500 velas de vencimiento, el horizonte (8 y 4 velas), el timeframe principal o los filtros F1 a F5;
- cambiar el diseño del placebo (mes, franja, distancia, tamaño, espera, 5 por evento) o la normalización por ATR;
- cambiar la etiqueta F de H5.2 (por ejemplo, pasar a "toque en `t` o `t − 1`"), filtrar por año, dirección, franja, tercil o gatillo (los cortes de §4.4 y §5.5 son descriptivos);
- cambiar los costos, el escenario de financiación (los dos votan en H5.2), el estrés, el corte de las mitades, los niveles, el N o los mínimos de muestra;
- pasar a 1 hora o a un solo índice como "el principal" porque dio mejor;
- mirar el fuera de muestra (incluidas las velas de IBKR) sin la aprobación del dueño;
- volver a correr con otra regla y presentar la mejor.

**Errores de código:** si después de ver los resultados aparece un error de código (no de la regla), se corrige, se vuelve a correr y se informa con fecha: cuál fue el error, el resultado anterior y el nuevo. El anterior no se borra (fila propia en el registro, con `compara_con`).

## 10. Al congelar y al correr

- **Congelar:** con las decisiones del dueño escritas en una sección nueva («Decisiones del dueño», con sus palabras y la fecha, citadas en #649), se quita "borrador", se commitea con fecha y hora y se agrega `2026-11-programa-5-fvg-preregistro.md.sha256`. En el mismo PR, una sección nueva **`[rule_program5]`** en `config/research.toml` (`alpha` = el nivel de cada hipótesis, `dsr_trials` = el N al congelar, `dsr_min = "0.95"`, `cost_stress = "1.5"`, `second_half_from = "2021-01-01"`) y **`[program5]`** con los parámetros de este archivo (timeframe, horizontes, placebos por evento, semilla, mínimos). Las secciones anteriores, `config/liquidity.toml`, `config/strategy.toml`, `config/risk.toml`, el runner y el paper **no cambian**.
- **Código:** un ejemplo nuevo `programa_5` en `engine/crates/backtest/examples/` que usa `fvg_created_at`, `is_filled_by` y `fvg_events` del motor (sin copiar la definición) y el `diagnostico` para H5.2. El sorteo del placebo y la etiqueta F son funciones puras con tests (incluidos no repintado, vencimientos y velas faltantes).
- **Correr:** cada corrida se anota con su hora (UTC) y su comando, y agrega su fila en `docs/analysis/registro-de-resultados.csv` en el mismo PR que su informe. Antes de calcular reacciones, la corrida imprime la cantidad de eventos reales, con placebo y sin placebo, por instrumento y año (y para H5.2, las operaciones F y R).
- **Si pasa:** se informa al dueño; él decide si se mira el fuera de muestra (una vez, con la regla congelada) y si se abre un Issue de spec. Pasar no habilita operar.
- **Si no pasa:** la hipótesis se cierra. No se prueban variantes sobre estos datos.

## 11. Puntos abiertos para el dueño

Todos **APROBADOS** por el dueño el 25-11-2026 con el valor recomendado: «apruebo los 20....» (#649, §13).

| # | Punto | Valor recomendado | Por qué | Estado |
|---|---|---|---|---|
| 1 | Presupuesto y reparto | 5 % nuevo para el programa 5: **H5.1 2,5 %, H5.2 2,5 %**, sin reserva | Solo dos hipótesis; partir más quita poder. Igual que los programas 2 y 4, con presupuesto propio | APROBADO (25-11-2026) |
| 2 | Timeframe principal | **15 m**; 1 h solo descriptivo | Es el de la estrategia (H5.2 lo necesita) y tiene 4 veces más eventos; probar los dos con voto pide partir el nivel | APROBADO (25-11-2026) |
| 3 | Tamaño mínimo del FVG | **Ninguno** (cualquier hueco > 0, la definición del motor); terciles de tamaño como descriptivo | No agrega un parámetro libre y prueba el FVG tal como lo usa hoy la estrategia. Alternativa: ≥ 0,25 ATR (deja ~60 % de los eventos) | APROBADO (25-11-2026) |
| 4 | Qué cuenta como toque | **Primer toque del motor** (`L ≤ techo`, `stats.rs`) **sin** la vela que en el mismo toque cierra más allá del piso (F2) | Es lo que mide hoy `[fvg_stats]`, y F2 es lo que la spec ya exige para "tocar" en el puntaje; se sabe al cierre de la vela | APROBADO (25-11-2026) |
| 5 | Espera mínima y máxima entre creación y toque | **Desde 1 vela** (puede ser la siguiente) **hasta 500** (`max_age_bars`) | Son los valores del motor; la espera va como descriptivo. Alternativa: ≥ 3 velas (deja ~40 %) | APROBADO (25-11-2026) |
| 6 | Horizonte principal | **8 velas** (2 h en 15 m); 4 velas como criterio de consistencia (> 0) | Son los dos que ya calcula el motor; uno solo vota para no partir el nivel, el otro tiene que ir en el mismo sentido | APROBADO (25-11-2026) |
| 7 | Vencimientos | **Excluir** el FVG si su creación, su toque o sus 8 velas de reacción cruzan un cambio de contrato (F1) | El salto entre contratos crea huecos y reacciones falsas; se pierde un 1 % de los eventos en 15 m y un 2–3 % en 1 h (§7) | APROBADO (25-11-2026) |
| 8 | Horario | **Todas las horas** en H5.1 (el motor calcula sobre todas las velas); contado / fuera como descriptivo y como condición de emparejamiento del placebo | Más muestra y es el FVG que ve hoy la liquidez; el placebo con la misma franja evita que el horario explique la diferencia | APROBADO (25-11-2026) |
| 9 | Diseño del placebo | El de §4.1: mismo mes y franja, misma distancia de llegada y tamaño en ATR, mismo primer toque con la misma espera, que no sea un toque real, **5 placebos por evento** | Copia todo menos el hueco; 5 por evento reduce el ruido del placebo sin costo de muestra | APROBADO (25-11-2026) |
| 10 | ATR de la reacción | **ATR[i] del motor** para los reales y ATR[s − 1] para los placebos; ATR[t − 1] para los reales como descriptivo | Usa la reacción tal como ya la guarda el motor; la diferencia juega en contra de H5.1 (conservador) | APROBADO (25-11-2026) |
| 11 | Estadística de H5.1 | Serie **mensual** de diferencias (real − placebo), **US100 y US500 combinados**, t unilateral con Newey-West de 3 rezagos | Los eventos de un mismo mes no son independientes; los programas anteriores también votan con series mensuales | APROBADO (25-11-2026) |
| 12 | Costos en H5.1 | **No votan**; se informa la diferencia en puntos contra el costo de ida y vuelta × 1 y × 1,5 | H5.1 no opera; los costos votan en H5.2 | APROBADO (25-11-2026) |
| 13 | Qué es "coincidir" en H5.2 | La vela de la señal `t` **es** el primer toque (con F1, F2 y F4) de un FVG de 15 m **de la misma dirección** | Lo más simple y sin parámetros; "en `t` o en `t − 1`" va como descriptivo | APROBADO (25-11-2026) |
| 14 | Qué vota en H5.2 | **El filtro relativo** (F contra el resto, §5.3); la estrategia filtrada contra comprar y mantener (alfa, Sharpe deflactado, 50/50) **solo descriptiva** | La estrategia en uso no tiene ventaja antes de costos (R001–R003): exigirle ganarle a comprar y mantener casi seguro gasta el 2,5 % sin aprender nada sobre el FVG | APROBADO (25-11-2026) |
| 15 | Muestras mínimas | H5.1: **1.000 eventos con placebo por instrumento**. H5.2: **100 operaciones F por instrumento** | 100 es el mínimo de los programas anteriores; en H5.1 sobra muestra (§7) y 1.000 evita pasar con un subconjunto chico | APROBADO (25-11-2026) |
| 16 | Conteo previo de H5.2 | **Autorizar** contar las operaciones F y R por año (sin P/L ni ningún resultado) antes de congelar; si F < 100 en algún instrumento, H5.2 no se congela y se avisa | Igual que la decisión 14 del programa 4: no gastar presupuesto en una hipótesis sin muestra | APROBADO (25-11-2026) |
| 17 | IBKR (MNQ y MES) | **No usarlo en este programa** | Todas sus velas (desde el 14-09-2026) están en el fuera de muestra; mirarlas sería gastar la prueba final. Si algo pasa, puede entrar en el único vistazo al fuera de muestra que apruebe el dueño | APROBADO (25-11-2026) |
| 18 | Generador y semilla | **SplitMix64, semilla 20261125** (la del programa 4) | Ya está definido y probado; fijarlo ahora evita elegir una semilla después | APROBADO (25-11-2026) |
| 19 | Orden de las pruebas | **H5.2 se corre aunque H5.1 no pase** (son independientes, cada una con su 2,5 %) | Una puede fallar y la otra no: el FVG puede no rebotar en general y aun así separar operaciones de la estrategia, o al revés | APROBADO (25-11-2026) |
| 20 | Congelar | Escribir las decisiones, quitar "borrador", SHA-256, `[rule_program5]` y `[program5]` en `config/research.toml`, y sumar H5.1 y H5.2 al N (§10) | El mismo procedimiento que los programas anteriores | APROBADO (25-11-2026) |

## 12. Referencias

- Spec §5.4 y §5.4.1 (`docs/spec/fase-1-especificacion.md`); `engine/crates/liquidity/src/fvg.rs` y `stats.rs`; `config/liquidity.toml` (`[fvg]`, `[fvg_stats]`, `[order_blocks]`).
- Pre-registros del programa 4 (#561), del programa Nasdaq (#526) y de la ronda 3 (control de M1).
- Diagnóstico de la estrategia en futuros de EE. UU. (#456), `docs/analysis/2026-11-diagnostico-us-futuros.md`.
- Bailey, D. H. y López de Prado, M. (2014). *The deflated Sharpe ratio*. Journal of Portfolio Management, 40(5), 94–107.
- Newey, W. K. y West, K. D. (1987). *A simple, positive semi-definite, heteroskedasticity and autocorrelation consistent covariance matrix*. Econometrica, 55(3), 703–708.
- Steele, G. L., Lea, D. y Flood, C. H. (2014). *Fast splittable pseudorandom number generators*. OOPSLA 2014.

## 13. Decisiones del dueño, conteo previo y precisiones fijadas al congelar

### 13.1 Decisiones del dueño

El 25-11-2026 el dueño (Alexei) escribió en #649, textual: **«apruebo los 20....»** (https://github.com/AlexeiSouyris/CFDsTrader/issues/649#issuecomment-7164689340). Se toma como la aprobación de los 20 puntos abiertos de §11 **con el valor recomendado de cada uno**, sin cambios:

1. 5 % nuevo: **H5.1 2,5 %, H5.2 2,5 %**, sin reserva. 2. **15 m** vota; 1 h solo descriptivo. 3. **Sin tamaño mínimo**. 4. Primer toque del motor **sin** la vela que llena en el mismo toque (F2). 5. Espera **de 1 a 500 velas**. 6. **8 velas** vota; 4 velas como consistencia (> 0). 7. **Excluir** los vencimientos de por medio (F1). 8. **Todas las horas**; la franja empareja el placebo. 9. Placebo de §4.1 con **5 por evento**. 10. **ATR[i]** para los reales, ATR[s − 1] para los placebos. 11. Serie **mensual**, US100 y US500 **combinados**, t unilateral con **Newey-West de 3 rezagos**. 12. Los costos **no votan** en H5.1. 13. "Coincidir" = la vela de la señal **es** el primer toque de un FVG de 15 m de la misma dirección (F1, F2, F4). 14. En H5.2 vota **el filtro relativo**; la estrategia filtrada es **solo descriptiva**. 15. Mínimos: **1.000 eventos con placebo** (H5.1) y **100 operaciones F** (H5.2) por instrumento. 16. Conteo previo de H5.2 **autorizado**. 17. **Sin IBKR**. 18. **SplitMix64, semilla 20261125**. 19. **H5.2 se corre aunque H5.1 no pase**. 20. Congelar como el programa 4.

**N para el Sharpe deflactado:** 359 (programa 4) + H5.1 + H5.2 = **361**; no se congeló otro programa entre medio. Valores en `config/research.toml`, secciones `[rule_program5]` y `[program5]`.

### 13.2 Conteo previo (punto 16), sin resultados

Corrido con `programa_5_conteo` (`engine/crates/backtest/examples/programa_5_conteo.rs`, commiteado antes de correr) el **25-11-2026 entre las 14:09 y las 14:10 UTC**, en una conexión de solo lectura, con velas hasta el 01-11-2025 (el fuera de muestra no se cargó). Usa las funciones del motor (`cfds_liquidity::fvg_events`) y los filtros de §3 (`cfds_backtest::research::program5`). **Solo cantidades**: no se calculó ningún P/L, ningún R, ninguna reacción ni ningún retorno.

**Eventos reales:** el conteo con el motor da **exactamente** las cifras de §7 (US100 15m: 59.636 → 59.174 → **45.731**; 1h: 15.064 → 14.720 → **11.074**; US500 15m: 51.101 → 50.738 → **41.436**; 1h: 13.669 → 13.355 → **10.516**), con los mismos totales por año.

**Operaciones de la estrategia en uso** en las 51 ventanas de validación (02-2013 → 10-2025), etiquetadas con §5.2:

| Instrumento | Escenario | Operaciones | F | R | F por año (2013 → 2025) |
|---|---|---|---|---|---|
| US100 | EUR | 2.692 | **577** | 2.115 | 38, 29, 54, 30, 41, 38, 48, 49, 44, 50, 57, 56, 43 |
| US100 | USD | 2.690 | **577** | 2.113 | igual que EUR |
| US500 | EUR | 2.606 | **566** | 2.040 | 37, 25, 47, 32, 37, 49, 50, 45, 37, 52, 53, 55, 47 |
| US500 | USD | 2.598 | **565** | 2.033 | igual que EUR salvo 2025: 46 |

Todas superan el mínimo de 100 operaciones F por instrumento: **H5.2 se congela**. (Los dos escenarios difieren en pocas operaciones porque la financiación cambia el capital dentro de cada bloque y, con eso, algún rechazo del riesgo o del kill switch.)

### 13.3 Precisiones de implementación fijadas al congelar

No cambian ninguna decisión aprobada; fijan cómo se calcula lo que §3 a §6 dejan implícito, antes de ver ningún resultado.

1. **Series y eventos.** Velas de `futures_candles` (`source = 'databento'`) de US100 y US500 con su `contract_id`, desde el 01-01-2011 (calentamiento) hasta el 01-11-2025 excluido. `fvg_events` del motor sobre toda la serie cargada (ATR de Wilder 14 de `wilder_atr` sobre toda la serie, como el motor y §7). **Un evento por FVG**: si una vela es el primer toque de varios FVG, cuenta como varios eventos. Orden de los eventos: hora del toque, luego vela de creación, luego alcista antes que bajista.
2. **Franja.** Una vela está "en contado" si empieza en o después de la apertura y termina en o antes del cierre de la sesión regular de XNYS de ese día (`config/sessions.toml` y los feriados y cierres anticipados de `config/holidays`); si no, está "fuera".
3. **Placebo.** Candidatas de un evento `e` con espera `a`: velas `s` con `s − a − 2 ≥ 0`, del mismo mes calendario (UTC, por la hora de inicio) y la misma franja que `t`, con ATR (el mismo `wilder_atr` de 14) > 0 en `s − 1`, las velas `s − a − 2` a `s + 8` del mismo contrato, `s + 8` antes del 01-11-2025, y las condiciones de toque, no llenado y primer toque de §4.1 calculadas en `Decimal`. La condición 3 (no es un evento real) se lee **literal**: se excluye `s` si es la vela de toque de un **evento real** (que pasó F1 a F3) de la misma dirección.
4. **Sorteo (un solo flujo, SplitMix64 semilla 20261125, índice = (x · n) >> 64).** Primero los 5 placebos de cada evento de 15 m (US100 en orden, luego US500); después las 1.000 repeticiones del test de suerte, cada una con 5 placebos por evento en el mismo orden; después, en el mismo flujo, los 5 placebos de cada evento de 1 h (descriptivo, sin test de suerte). Un evento sin candidatas no sortea nada y sale del análisis.
5. **Serie mensual de H5.1.** Mes = mes UTC del inicio de la vela del toque. `D_m` de un instrumento = media de `r_8` de sus eventos con placebo en `m` − media de `r_8` de todos sus placebos en `m`. Mitades y criterio 4 (`r_4`) con los mismos sorteos.
6. **Newey-West.** Con `n` meses y media `x̄`: `γ_l = (1/n) Σ_{j>l} (x_j − x̄)(x_{j−l} − x̄)`, `S = γ_0 + 2 Σ_{l=1..3} (1 − l/4) γ_l`, `t = x̄ / √(S/n)`, `p = P(T_{n−1} > t)`. Si `S ≤ 0`, no hay prueba y H5.1 no pasa.
7. **Test de suerte de H5.1.** En cada repetición, la media de `r_8` de todos los placebos (combinada y por instrumento); percentil = porcentaje de repeticiones con una media de placebo menor que la media real.
8. **Costos descriptivos de H5.1.** Ida y vuelta en puntos = 2 × (spread / 2 + 0,1 × spread + 1 tick), con el spread de `config/instruments.toml` en contado y el de `[program5] out_of_session_spread` fuera (US100 2,5; US500 1,6), y el tick de `config/instruments.toml`; × 1 y × 1,5. La diferencia en puntos de un evento = `r_8` real × ATR[i] − media de sus placebos de `r_8` × ATR[s − 1].
9. **Operaciones de H5.2.** Las de la estrategia en uso en cada una de las 51 ventanas de validación como bloque (capital inicial 10.000, calentamiento de `config/evaluation.toml`, noticias como el `diagnostico`, `evaluate_period_with`), con los dos escenarios de financiación (los CSV de `docs/analysis/2026-11-eur-eonia-estr.csv` y `2026-11-usd-effr-sofr.csv`). **R neto** = P/L neto / (tamaño × |entrada ejecutada − stop|), en la moneda del instrumento, como el `diagnostico`. **R antes de costos** = P/L a precio medio / el mismo riesgo. **Costos × 1,5** = (P/L a precio medio − 1,5 × costos totales) / riesgo, sobre las **mismas** operaciones (como los programas Nasdaq y 4). Mitades por la hora de la decisión.
10. **Prueba de H5.2.** Welch unilateral: `t = (m_F − m_R) / √(s_F²/n_F + s_R²/n_R)` con los grados de libertad de Welch-Satterthwaite; F y R juntan las operaciones de US100 y US500. Los criterios 2 a 4 comparan las medias de F y de R en cada subconjunto.
11. **"+1 de FVG u order block".** Una operación lo tuvo si su desglose del puntaje tiene el componente `FvgOrOrderBlock` con valor > 0.
12. **Test de suerte de H5.2.** Un flujo SplitMix64 nuevo con la semilla 20261125 por escenario (EUR y luego USD). Cada repetición sortea, sin reposición (Fisher-Yates parcial: se intercambia la posición `k` con `k + índice en [0, n − k)`), tantas operaciones de US100 como F tiene US100 y luego lo mismo en US500; estadístico = R neto medio de las sorteadas juntas; percentil = porcentaje de repeticiones con un valor menor que el de F.
13. **Estrategia filtrada (§5.4, descriptivo).** Se repite el recorrido de cada bloque permitiendo entradas solo en velas que son toque de un evento real (de cualquier dirección); si alguna operación abre en contra de todos los eventos de su vela, esa vela se quita y se repite, hasta que todas las operaciones son F. Retorno de cada operación = P/L neto (o con costos × 1,5 o × 2) en la moneda de la cuenta / capital del bloque al abrirla; retornos mensuales compuestos por mes de cierre; los 5 criterios de `alpha_verdict` contra comprar y mantener el CFD (como el programa 4 §7), con N = 361. No vota.
14. **Terciles de tamaño** (descriptivo): los del motor, es decir, los cortes de los terciles de `size_atr` de los eventos reales de cada instrumento y timeframe.
