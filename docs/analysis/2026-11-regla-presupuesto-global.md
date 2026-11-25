# Propuesta: presupuesto de error global, filtro de borde bruto y chequeo del N del Sharpe deflactado

Issue #660 · punto 2 del plan de `2026-11-autoanalisis.md` (#658) · 25-11-2026 · **propuesta: nada de esto rige hasta que el dueño lo apruebe** (tabla de §6).

Hecho por un agente de IA (Claude Code). No cambia ningún pre-registro congelado ni ningún valor de `config/`. Nada de esto es una recomendación de inversión.

## Resumen simple

- **Qué pasa hoy.** Cada programa de investigación abrió su propio 5 % de probabilidad de falso positivo. Ya se gastaron **18,75 %** en 13 hipótesis que votaron (y otros 10 % en las dos pruebas de ML). Ninguna pasó, así que **no hay ningún falso positivo declarado**. Pero si seguimos abriendo un 5 % por programa, tarde o temprano algo "pasa" por azar.
- **Qué se propone.** Un solo presupuesto para **todo el proyecto**, con el método de *alfa-inversión* (Foster y Stine, 2008): se arranca con 4,75 %; cada hipótesis paga lo que arriesga; si una hipótesis pasa **y el fuera de muestra la confirma**, devuelve un premio que permite seguir probando. Sin descubrimientos, alcanza para unas **4 hipótesis** a 1,25 % y después se acaba. No hay más "5 % nuevos".
- **Lo ya gastado** no se arrastra como deuda (con esta regla ya estaríamos en −14 %), pero **sigue contando** en el N del Sharpe deflactado, que nunca se reinicia.
- **Filtro de borde bruto.** Antes de gastar presupuesto, la hipótesis tiene que mostrar, **sin mirar resultados**, que su ventaja bruta esperada por operación es al menos **k = 2 veces** el costo total por operación (spread, slippage, comisión y financiación, todos de `config/`). Las intradía de 15 minutos casi nunca lo cumplen.
- **Chequeo en el CI.** Un script verifica que el último `dsr_trials` de `config/research.toml` cubra todas las pruebas del registro de resultados. **Hoy cuadra: 361 = 361.**

## 1. Lo gastado hasta hoy

Fuentes: los pre-registros congelados (`2026-11-ronda1-preregistro.md`, `2026-11-ronda2-preregistro.md`, `2026-11-ronda3-preregistro.md`, `2026-11-programa-nasdaq-preregistro.md`, `2026-11-programa-4-preregistro.md`, `2026-11-programa-5-fvg-preregistro.md`, `2026-11-ml-v2-preregistro.md`), `config/research.toml` y `registro-de-resultados.csv`.

| Programa | Hipótesis que votaron (fila) | Nivel por hipótesis | Gastado | p (el peor de los dos escenarios) |
|---|---|---|---|---|
| Programa 1, ronda 1 (#472) | C (R014), D1 (R015) | 1,25 % | 2,50 % | C: t −2,75 (p ≈ 1); D1: 0,31 |
| Programa 1, ronda 2 (#486) | A (R016), B (R017) | 1,25 % | 2,50 % | A: 0,18; B: t −11 (p ≈ 1) |
| Programa 2, ronda 3 (#493) | V (R019), M1 (R020) | 1,25 % | 2,50 % | V: 0,207; M1: t −2,37 (p ≈ 0,99) |
| Programa 2, reserva → Nasdaq (#526) | H1 (R027), H2 (R031) | 1,25 % | 2,50 % | H1: 0,125; H2: 0,641 |
| Programa 4 (#561) | H4.1 (R029), H4.2 (R030), H4.3 (R033) | 1,25 % | 3,75 % (reserva de 1,25 % sin usar) | 0,171; 0,954; 0,995 |
| Programa 5 (#649) | H5.1 (R035), H5.2 (R037) | 2,50 % | 5,00 % | 0,963; 0,663 |
| **Investigación, total** | **13 hipótesis** | | **18,75 %** | **ninguna pasa** |
| ML v1 y v2 (#430, #435) | R010; R011–R013 | 5 % y 5 %/3 | 10,00 % | ninguna mejora a B1 |
| Programa 3 (#513) | borrador, no congelado | — | 0 % | — |

Qué quiere decir:

- **Cota de Bonferroni** (la suma): 18,75 % en investigación; 28,75 % contando ML. Si las pruebas fueran independientes, la probabilidad de al menos un falso positivo sería 17,2 % (25,3 % con ML). Es lo que el autoanálisis llamó "20–25 %".
- **Con alfa-inversión** (§2.2), las 13 hipótesis habrían costado Σ α/(1 − α) = **19,05 %**: con 4,75 % de partida, el proyecto estaría en **−14,3 %**, sin poder probar nada más.
- **Con FDR sobre el registro** (§2.3), con m = 13: el umbral de Benjamini–Hochberg para el menor p es 0,05/13 = 0,0038, y el de Benjamini–Yekutieli 0,05/(13 × 3,18) = 0,0012. El menor p del registro es 0,125 (H1). **Ninguna hipótesis pasa con ningún método**: cambiar la regla no cambia ningún veredicto pasado.
- **N del Sharpe deflactado:** 350 → 352 → 354 → 356 → 359 → 361 (`[rule]` … `[rule_program5]`). Cuadra con el registro (§4).

## 2. Opciones comparadas

### 2.1 Gasto de alfa global (*alpha-spending*, Bonferroni sobre el proyecto)

Un 5 % total para todo el proyecto, repartido de antemano (por ejemplo, 1,25 % por hipótesis hasta agotarlo).

- A favor: es lo más simple y controla la probabilidad de **cualquier** falso positivo (FWER).
- En contra: con lo gastado (18,75 %) ya está agotado cuatro veces. Para seguir habría que reiniciar, y un reinicio es exactamente el "5 % nuevo" que se quiere evitar. No devuelve nada si algo pasa.

### 2.2 Alfa-inversión (Foster y Stine, 2008)

Foster, D. P. y Stine, R. A. (2008). *α-investing: a procedure for sequential control of expected false discoveries*. Journal of the Royal Statistical Society, Series B, 70(2), 429–444.

- Hay una "riqueza" de alfa `W`. Se arranca con `W(0) = α × η` (con α = 5 % y η = 1 − α = 0,95: **4,75 %**).
- Cada hipótesis `j` elige su nivel `α_j ≤ W/(1 + W)` y paga `α_j/(1 − α_j)`, pase o no.
- Si la hipótesis se rechaza (pasa), recibe un premio `ω ≤ α`.
- Controla el **mFDR** (falsos descubrimientos esperados sobre descubrimientos esperados + η) en α, en cualquier momento y aunque las hipótesis se elijan mirando las anteriores. Si todas las hipótesis son falsas (lo más probable en lo que venimos probando), todo descubrimiento es falso y la misma cuenta acota la probabilidad de al menos uno en α.
- A favor: es **secuencial**, como trabajamos (una hipótesis por vez, congelada antes de correr); el veredicto de cada prueba queda fijo al correrla; un acierto confirmado "paga" más pruebas; una racha de fracasos agota el presupuesto, que es la señal correcta.
- En contra: controla una tasa de descubrimientos falsos, no la probabilidad de cualquier falso positivo cuando hay hipótesis verdaderas mezcladas. Es menos conocida que Bonferroni.

Con apuestas de 1,25 %: después de la 1ª queda 3,48 %, después de la 2ª 2,22 %, después de la 3ª 0,95 %, y la 4ª solo puede apostar 0,94 % y deja el presupuesto en 0.

### 2.3 FDR sobre todo el registro (Benjamini–Hochberg o Benjamini–Yekutieli)

Benjamini, Y. y Hochberg, Y. (1995). *Controlling the false discovery rate*. JRSS-B, 57(1), 289–300. Benjamini, Y. y Yekutieli, D. (2001). *The control of the false discovery rate in multiple testing under dependency*. Annals of Statistics, 29(4), 1165–1188.

- Se ordenan los p de todas las hipótesis del registro y se rechazan las `i` primeras con `p_(i) ≤ i × q/m` (BH) o `≤ i × q/(m × Σ 1/k)` (BY, válido con cualquier dependencia; nuestros índices de EE. UU. se mueven casi igual).
- A favor: más potencia que Bonferroni cuando hay muchas hipótesis verdaderas.
- En contra: es **fuera de línea**. El veredicto de una hipótesis depende de las que se agreguen después: una idea que "pasa" hoy puede dejar de pasar mañana, o al revés. Eso choca con la regla de fijar el veredicto antes de correr. BY, además, es muy exigente con m chico.
- Uso propuesto: **auditoría descriptiva** del registro en cada informe, sin voto.

### 2.4 Solo el N del Sharpe deflactado

Bailey, D. H. y López de Prado, M. (2014). *The deflated Sharpe ratio*. Journal of Portfolio Management, 40(5), 94–107.

- Ya está: el N cuenta todas las pruebas y nunca se reinicia. Es la memoria de la búsqueda.
- En contra: solo corrige el Sharpe de una estrategia. No sirve para hipótesis que no son una estrategia (H5.1 compara reacciones con un placebo; H5.2 compara dos grupos de operaciones) y no le pone un techo a la cantidad de pruebas. Es un complemento, no un presupuesto.

### 2.5 Recomendación

**Alfa-inversión (2.2) con α = 5 % para todo el proyecto, más el N del Sharpe deflactado acumulado (2.4), más una auditoría BH/BY sin voto (2.3).**

- Una sola riqueza `W` para investigación **y** para las pruebas confirmatorias de ML.
- **Época nueva:** `W(0) = 4,75 %` desde la fecha en que el dueño apruebe. Lo gastado antes no se arrastra como deuda: ninguna hipótesis pasó, así que no hay ningún falso positivo declarado que proteger, y la búsqueda ya hecha sigue pesando en el N del Sharpe deflactado (hoy 361). Es la **última** época: no hay otra sin una decisión registrada del dueño (punto 6 de §6).
- Apuesta por defecto `α_j = mín(1,25 %, W/(1 + W))`. El pre-registro puede apostar menos, nunca más.
- Premio `ω = 5 %`, **pagado solo si el fuera de muestra confirma** la hipótesis (pagar menos o nada no rompe la garantía, porque `ω ≤ α`).
- Lo que no vota (descriptivos) sigue sin gastar. Una hipótesis congelada que después no se corre **paga igual** al congelarse.
- La reserva de 1,25 % del programa 4 y el 5 % pedido por el borrador del programa 3 dejan de existir: todo sale de `W`.
- El estado de `W` se lleva en el registro (§5).

## 3. Filtro de borde bruto

### 3.1 La regla

Antes de congelar una hipótesis que gaste presupuesto, su pre-registro tiene que traer una **estimación previa** de la ventaja bruta esperada por operación, `E_bruto`, y el **costo total por operación**, `C`, en las mismas unidades (puntos del instrumento, o R si la estrategia tiene stop). La hipótesis entra solo si:

**E_bruto ≥ k × C**, con **k = 2** (PENDIENTE).

- Por qué 2: las anomalías publicadas pierden en promedio un 26 % fuera de la muestra del artículo y un 58 % después de publicadas (McLean, R. D. y Pontiff, J. (2016). *Does academic research destroy stock return predictability?* Journal of Finance, 71(1), 5–32). Con k = 2, aun si el borde se reduce a la mitad, cubre los costos. Con menos que eso, lo esperable es perder.
- El filtro **no gasta presupuesto** y **no es una prueba**: si no se cumple, la hipótesis no se congela y queda anotada en el pre-registro o en el Issue como "descartada por el filtro", con sus números.

### 3.2 Costo por operación, desde `config/`

Por operación de ida y vuelta, en puntos, con `s_e` y `s_s` el spread en la franja horaria de la entrada y de la salida:

```
C = spread + slippage + comisión + financiación

spread      = (s_e + s_s) / 2
slippage    = (slippage_spread_fraction × s_e + slippage_ticks × tick)
            + (slippage_spread_fraction × s_s + slippage_ticks × tick)
comisión    = 0 en los CFD de índices (config/instruments.toml: "Los CFD de índices no tienen comisión");
              en acciones, la de [share_costs], convertida a puntos
financiación = noches × P × (tipo_de_referencia + funding_admin_fee_pct) / 100 / funding_day_count   (largos)
             = 0 para cortos (lo que se cobra por estar corto NO se cuenta como borde)
```

- `slippage_spread_fraction = 0,1` y `slippage_ticks = 1` (`config/backtest.toml`, `[costs]`).
- `s_e`, `s_s`: `spread_typical_points` de `config/instruments.toml` en la sesión de contado; fuera de ella, el spread de la franja de `docs/research/ig-spreads-2026-11.md` (por ejemplo `out_of_session_spread` de `config/research.toml`: US100 2,5, US500 1,6). Si falta el dato, se usa el mayor publicado y se dice.
- Financiación: `funding_benchmark` y `[funding_benchmarks]` (SOFR 3,94 para US500, US100 y US30; €STR 2,441 para DE40), `funding_admin_fee_pct = 3`, `funding_day_count = 360`. `noches` = las esperadas por operación según la regla (0 si cierra en el día).
- `P`: precio de referencia del período de la hipótesis (por ejemplo, la mediana del cierre en la validación). Es un dato descriptivo, no un resultado.
- Con estrés: el pre-registro informa también `C × cost_stress` (1,5), sin voto.

Ejemplos con los valores de hoy (intradía, sesión de contado, 0 noches):

| Instrumento | spread | slippage (ida y vuelta) | C |
|---|---|---|---|
| US100 | 1,6 | 2 × (0,16 + 0,1) = 0,52 | **2,12 puntos** |
| US500 | 0,9 | 2 × (0,09 + 0,01) = 0,20 | **1,10 puntos** |
| US30 | 3,7 | 2 × (0,37 + 0,1) = 0,94 | **4,64 puntos** |

Una noche en largo cuesta `P × 6,94 % / 360`: con un US100 de ejemplo en 20 000 puntos, **3,86 puntos por noche**, casi el doble del spread.

### 3.3 Qué cuenta como estimación previa

Vale una de estas, escrita en el pre-registro **antes** de mirar resultados:

1. **Literatura:** el efecto bruto publicado (con cita, período y mercado), convertido a puntos por operación en nuestro instrumento y horizonte.
2. **Descriptivos sin resultados:** estadísticas de los datos que **no** usan el resultado de la regla, por ejemplo el desvío del retorno en el horizonte de la operación, `σ_h`, y la frecuencia de señales. La cota es `E_bruto ≈ IC × σ_h` (Grinold, R. C. y Kahn, R. N. (2000). *Active Portfolio Management*, 2.ª ed., McGraw-Hill), con un IC (correlación entre la señal y el retorno) previo **de la literatura**, o **0,05** si no hay cita. Con eso, la regla pide `IC ≥ k × C / σ_h`.

No vale: el P/L, el porcentaje de aciertos o cualquier cifra de la regla sobre los datos de la prueba; ni ajustar el IC hasta que pase.

Ejemplos (ilustran la regla; usan cifras que ya estaban en informes):

- Deriva nocturna (programa 2 §1.5): 1,48 pb por día publicados contra 3–4 pb de costo: cociente ≤ 0,5. **No entra**, y es lo que se hizo a mano.
- Estrategia en uso de 15 m (R001–R003): antes de costos −0,02 a +0,01 R contra 0,13–0,18 R de costo. No habría entrado (aunque esa cifra es un resultado, no una estimación previa).

## 4. El N del Sharpe deflactado y el chequeo del CI

### 4.1 Regla de conteo

```
N esperado = 344
           + filas del registro con programa = "ML"
           + filas de hipótesis que votan: programa empieza con "programa", no dice "descriptivo",
             y el veredicto no dice "reemplazada" (una corrida inválida reemplazada cuenta una sola
             vez, en la fila que la reemplaza)
```

- **344** = las pruebas de antes del registro, según el pre-registro de la ronda 1 §2.5: ~300 cortes de #461, 11 variantes del DE40 (#373) y los walk-forward de la estrategia en uso (R001–R009 son de ese grupo), redondeado hacia arriba. Sale de 350 = 344 + ML v1 + 3 familias de ML v2 + C + D1.
- No suman: los descriptivos sin voto, la referencia (R018) y las filas de la estrategia en uso (ya están en el 344; ver punto 11 de §6 para las futuras).

### 4.2 Cuenta de hoy

`344 + 4 ML (R010–R013) + 13 hipótesis (R014, R015, R016, R017, R019, R020, R027, R029, R030, R031, R033, R035, R037) = 361`, y el último `dsr_trials` es `[rule_program5] = 361`. **Cuadra.** R034 (la corrida de H5.1 con un error de código) no suma: la reemplaza R035. Cada sección anterior también cuadra con su momento: 350 (C, D1), 352 (+A, B), 354 (+V, M1), 356 (+H1, H2), 359 (+H4.1–H4.3), 361 (+H5.1, H5.2).

### 4.3 El chequeo

`tools/research/check_dsr_trials.py` (solo biblioteca estándar de Python), en el job `registro (dsr_trials)` del CI, que entra en `CI ok`:

- **Falla** si el registro tiene más pruebas que el último `dsr_trials` (una prueba se registró sin sumarla al N), o si algún `dsr_trials` baja frente al anterior en el archivo.
- **Pasa con aviso** si `dsr_trials` es mayor: hipótesis congeladas que todavía no tienen fila (pasa en el PR que congela un programa, antes de correrlo).
- Tiene tests (`tools/research/test_check_dsr_trials.py`), incluido uno que corre sobre los archivos reales del repo.

## 5. Cómo se lleva el presupuesto (si se aprueba)

- Una sección nueva en `registro-de-resultados.md`, "Presupuesto global", con una línea por hipótesis congelada: fecha, hipótesis, `α_j`, costo `α_j/(1 − α_j)`, premio (si hubo) y `W` después. Sin cambiar las filas viejas del CSV.
- El pre-registro de cada hipótesis cita `W` antes y después, su filtro de borde bruto (§3) y el N al congelar.
- Una vez aprobado, el chequeo del CI puede sumar que `W` nunca quede negativo (otro PR).

## 6. Puntos para el dueño

| # | Punto | Recomendación | Estado |
|---|---|---|---|
| 1 | Reemplazar "5 % nuevo por programa" por un presupuesto único del proyecto | Sí | PENDIENTE |
| 2 | Método | Alfa-inversión de Foster y Stine (2008), α = 5 % | PENDIENTE |
| 3 | Riqueza inicial | `W(0) = 4,75 %` (α × η, η = 0,95), desde la fecha de aprobación | PENDIENTE |
| 4 | Lo ya gastado (18,75 % en investigación, 10 % en ML; 19,05 % con el costo de alfa-inversión) | No se arrastra como deuda; queda registrado en este archivo y sigue contando en el N del Sharpe deflactado | PENDIENTE |
| 5 | Apuesta por hipótesis y premio | `α_j = mín(1,25 %, W/(1 + W))`; premio `ω = 5 %` solo si el fuera de muestra confirma | PENDIENTE |
| 6 | Épocas nuevas | Ninguna más sin una decisión registrada del dueño en un Issue, con el motivo | PENDIENTE |
| 7 | Alcance | Investigación **y** pruebas confirmatorias de ML salen de la misma `W`; los descriptivos sin voto no gastan; una hipótesis congelada paga aunque no se corra | PENDIENTE |
| 8 | Reserva del programa 4 (1,25 %) y 5 % del borrador del programa 3 | Dejan de existir; todo sale de `W` | PENDIENTE |
| 9 | Auditoría FDR | BH y BY sobre todo el registro en cada informe, **sin voto** | PENDIENTE |
| 10 | Regla de conteo del N (§4.1) con base 344 | Sí, y el chequeo del CI de este PR la hace cumplir | PENDIENTE |
| 11 | Filas futuras de la estrategia en uso (cambios de parámetros, walk-forwards nuevos) | Que sumen 1 al N cada una desde la aprobación (cambio del script en otro PR) | PENDIENTE |
| 12 | Filtro de borde bruto obligatorio antes de congelar | Sí | PENDIENTE |
| 13 | Factor k | **2** | PENDIENTE |
| 14 | Costo por operación | Fórmula de §3.2, solo con valores de `config/`; spread de la franja horaria; financiación de largos; lo que se cobra por estar corto no cuenta | PENDIENTE |
| 15 | Estimación previa válida | Literatura con cita, o `IC × σ_h` con IC de la literatura o 0,05 si no hay cita (§3.3) | PENDIENTE |
| 16 | Dónde se lleva `W` | Sección "Presupuesto global" en `registro-de-resultados.md` (§5) | PENDIENTE |

## 7. Supuestos y riesgos

- Las condiciones de alfa-inversión (`W(0) ≤ α η`, costo `α_j/(1 − α_j)`, `ω ≤ α`) están tomadas del artículo de Foster y Stine (2008) sin poder leer el texto completo en esta sesión. **Conviene verificarlas contra el artículo antes de aprobar.**
- Los p de la tabla de §1 son los del registro; cuando el informe da solo el t (C, B, M1), el p unilateral se infiere del signo y del t, y en los tres es ≈ 1.
- La base 344 es la estimación redondeada de la ronda 1, no un conteo exacto. El chequeo del CI la toma como fija.
- `IC × σ_h` es una aproximación de orden de magnitud: sirve para descartar ideas sin chance, no para estimar ganancias.
- Una época nueva es, en la práctica, un último "5 % nuevo". El freno es que no hay otra sin decisión registrada, y que el N del Sharpe deflactado sigue creciendo.
