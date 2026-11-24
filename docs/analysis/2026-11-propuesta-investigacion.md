# Propuesta de investigación: qué probar después de la estrategia actual (pre-registro)

Issue #462 · 24-11-2026 · **propuesta para el dueño**: no cambia `config/`, ni código, ni la spec. Cada familia de abajo, si el dueño la elige, lleva **su propio Issue, su pre-registro commiteado antes de correr y su aprobación**. Nada de esto es una recomendación de operar: son hipótesis para medir, y los resultados pasados no prometen ningún resultado futuro.

## 1. Punto de partida

Los estudios de esta semana coinciden:

| Estudio | Qué mostró |
|---|---|
| Diagnóstico DE40 (#323, PR #324, `2026-11-diagnostico-de40-futuros.md`) | Sin ventaja **antes** de costos (−1685 a precio medio), costos ≈ 0,04 R por operación, el 63 % termina en el stop |
| Variantes DE40 (#373, `2026-11-variantes-de40.md`) | Ninguna de 11 variantes se separa de la base; no hay tramo limpio para confirmar (faltan años de FDAX) |
| Walk-forward EE. UU. (#411, #457) | US500 y US30 pierden ~7–8 % por ventana; US100 con 51 ventanas también pierde (−6,1 %); US2000 pierde con todos los supuestos; la tasa de financiación no cambia el veredicto |
| Diagnóstico EE. UU. (#461, en revisión) | A precio medio ≈ 0 R (−0,02, +0,01, −0,05 R por operación); los costos se llevan **0,18 / 0,13 / 0,10 R** por operación (US500 / US100 / US30); ningún filtro da vuelta el signo en los tres índices |
| ML v1 y v2 (#430, #435) | AUC 0,50–0,53: ninguna variable (contexto, tasas, VIX, análogos) predice TP1 mejor que la tabla de §13 |

**Lectura:** el problema no es un parámetro. La estrategia opera en 15m con stops de ~1–1,5 ATR de 15m (unos 8–10 puntos en US500). Con 1,1 puntos de costo por ida y vuelta, el costo es una parte fija y grande del R, y la señal no tiene ventaja antes de costos que lo pague. Seguir filtrando la misma estrategia gasta comparaciones sobre los mismos datos (ver la advertencia de #461: ~300 comparaciones).

Esta propuesta cambia **la pregunta**: buscar familias con una hipótesis económica publicada, **distintas** de la confluencia de 15m, y con un costo por operación que sea una fracción chica de lo que se espera ganar.

## 2. Lo que cuesta operar (la vara para cualquier familia)

### 2.1 Costos por ida y vuelta (de `config/instruments.toml`, sin cambios)

Spread típico (la mitad en cada ejecución) + slippage supuesto de la spec §11 (0,1 × spread + 1 tick, en cada ejecución). Sin comisión en índices.

| Instrumento | Spread | Tick | Ida y vuelta (puntos) | Financiación por noche, largo, a 5 % anual* | a 7 % anual* |
|---|---|---|---|---|---|
| US500 | 0,9 | 0,01 | **1,1** | 0,61 | 0,85 |
| US100 | 1,6 | 0,1 | **2,1** | 2,05 | 2,87 |
| US30 | 3,7 | 0,1 | **4,6** | 4,77 | 6,68 |
| DE40 | 1,8 | 0,1 | **2,4** | 3,32 | 4,65 |
| US2000 | supuesto (#421): 0,43–0,9 | 0,1 | 0,7–1,3 | — | — |
| XAUUSD | mínimo 0,9; típico, valor por punto y tamaño mínimo **pendientes** | — | — | — | — |

\* Financiación de IG: noches × precio × tamaño × (referencia ± 3 % de tasa administrativa) / 360. Los largos pagan referencia + 3 %; los cortos reciben referencia − 3 % (si es negativo, también pagan). "5 %" ≈ €STR 2 % + 3 %; "7 %" ≈ SOFR 4 % + 3 %. El tipo de referencia de los CFD de EE. UU. sigue **pendiente** de confirmar con IG. Precio = mediana del cierre diario del futuro 2020-01 → 2025-10 (US500 4373, US100 14 756, US30 34 361; DE40 23 909, 2025-03 → 2025-10).

### 2.2 Volatilidad de referencia (medida para esta propuesta)

Para estimar el costo en R se midieron **solo magnitudes de volatilidad** (rangos), no el resultado de ninguna regla, en los futuros de 2020-01 a 2025-10 (antes del fuera de muestra, conexión de solo lectura):

| Instrumento | Rango diario mediano | Rango 15m mediano (sesión de NY) | Rango de los primeros 30 min de NY (mediana) | Movimiento absoluto de los últimos 30 min (mediana) |
|---|---|---|---|---|
| US500 | 57 (1,28 %) | 7,8 | 16,8 | 6,3 |
| US100 | 262 (1,73 %) | 35,3 | 93,0 | 23,5 |
| US30 | 424 (1,19 %) | 58,0 | 155,0 | 42,0 |
| US2000 | 39 (1,98 %) | 5,2 | 16,1 | 3,4 |
| DE40 (2025) | 290 (1,24 %) | 37,0 (Xetra) | — | — |

Control: con un stop de ~1–1,3 rangos de 15m, el costo de US500 da 1,1 / (8–10) ≈ 0,11–0,14 R, más la financiación. Coincide con los 0,18 R medidos en #461 (que incluye años con el índice más bajo y el mismo spread en puntos).

### 2.3 La vara

- **Objetivo de costo:** ≤ 0,05 R por operación (un tercio o menos de lo de hoy), o, en familias sin un R natural, costo ≤ 10 % del movimiento medio que la señal intenta capturar.
- **Tamaño mínimo:** con 10 000 € y 1 % de riesgo (100 €), un stop en puntos mayor que 100 / tamaño mínimo **no se puede abrir**: US500 > 100 puntos, US100 > 200, US30 > 500, DE40 > 200. Las familias de varios días chocan con esto (ver A).
- **Base del futuro en tenencias de varios días:** las velas son de futuros sin ajustar (ADR 0011) y el backtest les suma la financiación de IG. Dentro de un contrato, el futuro converge al contado, así que su deriva ya descuenta (tasa − dividendos). En tenencias de varios días, sumarle además la financiación completa del CFD **podría contar dos veces la tasa** de referencia, y no está confirmado si IG ajusta los dividendos en sus CFD de índices. Intradía no importa. **Antes de probar A, D o E hay que resolver esto** en un Issue aparte (ver §5).

## 3. Familias de hipótesis

Cada familia se define con reglas medibles y sin parámetros elegidos mirando los datos: los valores salen de la fuente citada. Todas pasan por el módulo de riesgo (§10 de la spec: 1 % por operación, stop obligatorio, −3 % diario, −15 % kill switch). Todas deciden al **cierre** de una vela y ejecutan en la apertura de la siguiente (CLAUDE.md §5b). El horario de NY es el de la bolsa (America/New_York), así que el cambio de horario se resuelve solo.

### A. Seguimiento de tendencia de baja frecuencia (time-series momentum)

**Fuentes.**
- Moskowitz, T. J., Ooi, Y. H. y Pedersen, L. H. (2012). *Time series momentum*. Journal of Financial Economics, 104(2), 228–250.
- Hurst, B., Ooi, Y. H. y Pedersen, L. H. (2017). *A century of evidence on trend-following investing*. Journal of Portfolio Management, 44(1), 15–29.
- Faber, M. T. (2007). *A quantitative approach to tactical asset allocation*. Journal of Wealth Management, 9(4), 69–79.
- En contra: Sullivan, R., Timmermann, A. y White, H. (1999). *Data-snooping, technical trading rule performance, and the bootstrap*. Journal of Finance, 54(5), 1647–1691. Las reglas de medias móviles sobre el Dow Jones de Brock, Lakonishok y LeBaron (1992, Journal of Finance, 47(5), 1731–1764) no superaron el control de data snooping en el período posterior.

**Definición (la de Moskowitz et al., sin elegir parámetros).** El último día hábil de cada mes, al cierre de la sesión: si el retorno de los últimos 12 meses (medido dentro de cada contrato y encadenado por el rollover) es > 0, largo; si es < 0, corto. Se mantiene hasta el mes siguiente. Stop de protección a 3 × ATR(20) diario; si salta, se queda afuera hasta la próxima revisión mensual. Respeta el cierre de los viernes de la spec §8: cierra el viernes y reabre el lunes en la apertura si la señal sigue.

**Por qué podría sobrevivir a los costos.** Opera ~12 veces por año y por instrumento (más las reaperturas semanales). Con un stop de ~3 rangos diarios (US500 ≈ 170 puntos), el spread de entrada y salida es ≈ 0,006 R. **El costo real es la financiación:** un largo paga ≈ 4–6 % anual del nocional en financiación más reaperturas semanales (US500: 4 noches × 0,61 + 1,1 por semana ≈ 4,2 % del precio por año a 5 %). Es del mismo orden que el retorno anual esperado de un índice de acciones. Los cortos reciben referencia − 3 % (con tasas bajas, también pagan). Cerrar los viernes es **más barato** que mantener el fin de semana (3 noches de financiación cuestan más que un spread).

**Riesgos y límites.**
- La evidencia más fuerte es en carteras diversificadas de decenas de futuros. Acá habría 3 índices de EE. UU. muy correlacionados (+ oro cuando estén sus costos): la diversificación es mucho menor.
- **Tamaño mínimo:** con 1 % de riesgo hace falta un capital de ~17 000 € en US500, ~39 000 € en US100 y ~25 000 € en US30 para poder abrir con ese stop. Con 10 000 € no se puede. Habría que decidir el capital del backtest o el riesgo por operación (decisión del dueño).
- Cambia la spec: timeframe diario, salidas propias (sin TP1/trailing/salida por tiempo de §8) y `stop_distance.max_atr`, que hoy se mide en ATR de 15m.
- Poca muestra: ~150 decisiones mensuales por instrumento en la validación. El contraste se hace sobre retornos diarios (ver §4).

**Datos.** Diarias armadas desde los futuros 1m: ES, NQ, YM desde 2011 (validación 2013-02 → 2025-10), RTY desde 2017, oro (GC) desde 2011 cuando se confirmen sus costos. DE40 no alcanza (desde 2025-03).

### B. Momentum intradía del mercado (primera media hora → última media hora)

**Fuentes.**
- Gao, L., Han, Y., Li, S. Z. y Zhou, G. (2018). *Market intraday momentum*. Journal of Financial Economics, 129(2), 394–414 (S&P 500, ETF SPY).
- Baltussen, G., Da, Z., Lammers, S. y Martens, M. (2021). *Hedging demand and market intraday momentum*. Journal of Financial Economics, 142(1), 377–403 (decenas de futuros; lo atribuyen a la cobertura de gamma de los intermediarios).

**Definición (la de Gao et al.).** Señal = signo del retorno desde el cierre del contado del día anterior (16:00 ET) hasta las 10:00 ET. Entrada en la apertura de la vela de 15:30 ET en la dirección de la señal; salida al cierre de la vela de 15:45 ET (16:00 ET). Stop de protección a 2 × ATR(15m). Una operación por día y por instrumento. Los viernes no se opera (la spec §2 no deja abrir en los últimos 30 minutos del viernes).

**Por qué podría sobrevivir a los costos.** No hay financiación (cierra en el día). Pero el costo es grande frente a lo que se captura: es el **17 %** del movimiento mediano de la última media hora en US500, el 9 % en US100 y el 11 % en US30. Aproximando con la mediana, para cubrir costos hay que acertar la dirección más del ~59 % de las veces en US500, ~55 % en US100 y ~56 % en US30. Es una vara alta: la previsibilidad publicada es estadísticamente fuerte, pero chica. **Prioridad baja en US500; posible en US100.**

**Datos.** Velas de 15m de los futuros, 2011 → 2025-10 (ES, NQ, YM) y RTY desde 2017. La señal usa precios de futuros fuera del horario del contado: sirve, porque el CFD de IG también cotiza fuera de hora.

### C. Ruptura del rango de apertura (opening range breakout)

**Fuentes.**
- Crabel, T. (1990). *Day Trading with Short Term Price Patterns and Opening Range Breakout*. Traders Press (libro de practicante, no revisado por pares).
- Holmberg, U., Lönnbark, C. y Lundström, C. (2013). *Assessing the profitability of intraday opening range breakout strategies*. Finance Research Letters, 10(1), 27–33 (futuros de petróleo crudo; encuentran rentabilidad significativa en su muestra).

**Definición.** Rango de apertura = máximo y mínimo de las dos primeras velas de 15m del contado (09:30–10:00 ET; para DE40, 09:00–09:30 Europe/Berlin). La primera vela de 15m que **cierra** por encima del máximo abre un largo en la apertura de la vela siguiente (simétrico para cortos). Stop en el extremo opuesto del rango. Salida al cierre de la última vela de 15m antes del cierre de la sesión. Una sola operación por día y por instrumento; si no rompe antes de las 15:00 ET, no se opera. Sin TP ni trailing: la definición no se toca después de mirar.

**Por qué podría sobrevivir a los costos.** El R es el rango de apertura completo (o más), mucho más ancho que el stop actual: **costo ≈ 0,065 R en US500, 0,023 R en US100 y 0,030 R en US30**, sin financiación (cierra en el día). Es la familia con mejor relación costo/R y la que más operaciones da: hasta ~250 por año y por instrumento, ~3000 en la validación. Con esa muestra, un efecto de 0,05 R por operación es detectable aun con la corrección (§4). El tamaño entra en el riesgo (US100: 100 € / 93 puntos ≈ 1,07 € por punto ≥ 0,5 mínimo). La spec ya prohíbe abrir en los primeros 15 minutos: la ruptura llega después de los 30.

**Riesgos.** La evidencia académica en índices de acciones es más débil que en A o D: la fuente principal es un practicante y el estudio revisado es en petróleo. El stop en el extremo opuesto puede superar los 3 ATR de 15m de `stop_distance.max_atr` en días de apertura violenta (US100: rango 93 / ATR 35 ≈ 2,6 en la mediana). Hay que decidir si el riesgo rechaza esos días (más simple) o si la familia lleva su propio máximo (cambio de config).

**Datos.** Velas de 15m de los futuros (ES, NQ, YM desde 2011; RTY desde 2017). DE40 solo cuando haya FDAX anterior a 2025 (#373).

### D. Estacionalidad de calendario (cambio de mes y antes de la Fed)

**Fuentes.**
- Cambio de mes: Ariel, R. A. (1987). *A monthly effect in stock returns*. Journal of Financial Economics, 18(1), 161–174. Lakonishok, J. y Smidt, S. (1988). *Are seasonal anomalies real? A ninety-year perspective*. Review of Financial Studies, 1(4), 403–425. McConnell, J. J. y Xu, W. (2008). *Equity returns at the turn of the month*. Financial Analysts Journal, 64(2), 49–64.
- Antes de la Fed: Lucca, D. O. y Moench, E. (2015). *The pre-FOMC announcement drift*. Journal of Finance, 70(1), 329–371.
- Advertencia general: McLean, R. D. y Pontiff, J. (2016). *Does academic research destroy stock return predictability?* Journal of Finance, 71(1), 5–32 (las anomalías publicadas pierden parte de su rendimiento después de publicarse). Hay que **esperar un efecto menor** que el de los artículos, que usan muestras anteriores a 2011.

**Definición D1 (cambio de mes, ventana de McConnell y Xu: del último día hábil del mes al tercero del mes siguiente).** Largo en la apertura de la última vela de 15m del contado del último día hábil del mes; salida al cierre del contado del tercer día hábil del mes nuevo. Stop de protección a 2 × ATR(20) diario. Si la ventana cruza un fin de semana, se respeta el cierre del viernes de la spec y se reabre el lunes. Calendario de feriados de `config/holidays` (se conoce de antemano: no hay look-ahead).

**Definición D2 (antes de la Fed).** Largo a las 14:00 ET del día hábil anterior a cada reunión programada del FOMC, salida a las 13:45 ET del día del anuncio (antes de las 14:00 ET). Solo reuniones programadas, con la fecha publicada antes del día de la entrada.

**Por qué podría sobrevivir a los costos.** Muy pocas operaciones: D1 ~12 por año, D2 ~8 por año. D1 en US500: 1,1 de ida y vuelta + 4 noches × 0,61 ≈ 3,5 puntos, frente a un stop de ~115 puntos ≈ **0,03 R**. D2: 1,1 + 0,61 ≈ 1,7 puntos frente a un stop de ~57 ≈ **0,03 R**. Para cubrir costos alcanza con que el retorno medio de la ventana supere ≈ 0,08 % (D1) o ≈ 0,04 % (D2) del precio.

**Riesgos.** Muestra chica: ~150 eventos de D1 y ~100 de D2 por instrumento en la validación, y los tres índices de EE. UU. se mueven casi igual en esos días (no son tres muestras independientes). El D1 en US500 cae justo en el límite del tamaño mínimo con 10 000 € (stop ≈ 115 > 100 puntos). Necesita la corrección de la base (§2.3). D2 necesita las fechas oficiales del FOMC desde 2011 (Federal Reserve, publicadas): hoy no están cargadas y cargarlas es un Issue de datos.

**Datos.** Diarias y 15m de los futuros, `config/holidays`; para D2, el calendario oficial del FOMC.

### E. Reversión a la media de corto plazo (diaria)

**Fuentes.**
- Connors, L. y Alvarez, C. (2008). *Short Term Trading Strategies That Work*. TradingMarkets Publishing (libro de practicante, no revisado por pares).
- Evidencia mixta: Lo, A. W. y MacKinlay, A. C. (1988). *Stock market prices do not follow random walks: evidence from a simple specification test*. Review of Financial Studies, 1(1), 41–66, encontraron autocorrelación **positiva** de los retornos semanales de índices en 1962–1985, lo contrario de una reversión.

**Definición (la regla del RSI de 2 períodos del libro).** Largo cuando el cierre diario está por encima de su SMA(200) y el RSI(2) diario cierra por debajo de 10; salida cuando el cierre supera la SMA(5). Stop de protección a 2 × ATR(20) diario (el libro no usa stop; la spec lo exige, y eso cambia la regla). Solo largos.

**Por qué podría sobrevivir a los costos.** Tenencias de 2 a 5 días: US500 ≈ 1,1 + 3 × 0,61 ≈ 2,9 puntos frente a un stop de ~115 ≈ **0,025 R**. La cantidad de operaciones por año no se conoce sin mirar los datos (se reporta, no se ajusta).

**Riesgos.** La fuente más débil de las cinco. El stop obligatorio cambia una regla pensada sin stop. Mismo problema de tamaño mínimo y de la base que D1. **Prioridad más baja.**

### Familias que se descartan (y por qué)

- **Deriva nocturna** (Lou, D., Polk, C. y Skouras, S. (2019). *A tug of war: overnight versus intraday expected returns*. Journal of Financial Economics, 134(1), 192–213; Boyarchenko, N., Larsen, L. C. y Whelan, P. (2023). *The overnight drift*. Review of Financial Studies, 36(9)). Comprar al cierre y vender a la apertura todos los días cuesta en US500 ≈ 1,1 + 0,61 ≈ 1,7 puntos por noche, ≈ 430 puntos por año, ≈ **10 % del precio por año**. Ningún efecto publicado en índices es de ese tamaño. Con los costos de un CFD no se puede.
- **Carry** (Koijen, R. S. J., Moskowitz, T. J., Pedersen, L. H. y Vrugt, E. B. (2018). *Carry*. Journal of Financial Economics, 127(2), 197–225). Es una estrategia **entre** muchos activos. Con 3–5 instrumentos no hay sección cruzada, y en un CFD el carry es justamente la financiación que se paga (referencia ± 3 %).
- **Más filtros o variables para la estrategia actual.** #461 ya miró ~300 cortes y ML v2 tres familias de variables: no hay ventaja antes de costos que filtrar. Cada intento nuevo consume la corrección (§4) sin una hipótesis nueva.

## 4. Regla de decisión pre-registrada (común a todas las familias)

Se fija **ahora**, antes de correr nada. Cambiarla después de ver un resultado invalida la familia, que se registra como una prueba nueva.

### 4.1 Datos y separación

- Walk-forward de `config/data_split.toml`: 24 meses de entrenamiento, 3 de validación, paso 3. Calentamiento de `config/evaluation.toml`.
- **Fuera de muestra intocable:** desde el 01-11-2025 (12 meses). No se carga, no se mira, no se usa para ajustar. Se usa **una sola vez**, para **un solo** candidato congelado, y solo si el dueño lo aprueba después de ver la validación.
- Las familias A–E no tienen parámetros que ajustar (vienen de la fuente). El entrenamiento solo sirve para el calentamiento; el resultado es la **concatenación de las ventanas de validación** (2013-02 → 2025-10 para ES, NQ y YM; 2019-08 → 2025-10 para RTY).
- Costos de config (spread, slippage), financiación en los **dos** escenarios empalmados ya usados (EONIA→€STR y EFFR→SOFR, #421) y un **estrés de costos × 1,5** (el spread histórico de IG no está medido, #267).

### 4.2 Unidad de prueba y corrección por comparaciones múltiples

- **Una hipótesis por familia**: la cartera con igual riesgo en US500, US100 y US30 (los tres con costos de IG y 15 años). US2000 (costos supuestos), DE40 (sin historia) y oro (costos pendientes) se reportan aparte, **descriptivos**, sin voto.
- **Presupuesto de error total: 5 % para todo el programa**, repartido por rondas (Bonferroni dentro de cada ronda):
  - Ronda 1 (C y D1): 2,5 % → **1,25 % por familia**, unilateral.
  - Ronda 2 (las que el dueño elija entre A, B, D2 y E): el 2,5 % restante dividido por las que entren.
  - Una familia que se agrega después o se "corrige" cuenta como una prueba más y sale del presupuesto que quede.
- Cada corrida, aunque falle, se registra (como `ml_experiments` en ML) para el **Sharpe deflactado** de la fase 5 (Bailey, D. H. y López de Prado, M. (2014). *The deflated Sharpe ratio: correcting for selection bias, backtest overfitting, and non-normality*. Journal of Portfolio Management, 40(5), 94–107) y la **probabilidad de sobreajuste** (Bailey, D. H., Borwein, J., López de Prado, M. y Zhu, Q. J. (2017). *The probability of backtest overfitting*. Journal of Computational Finance, 20(4), 39–69). Referencias del problema: Harvey, C. R., Liu, Y. y Zhu, H. (2016). *…and the cross-section of expected returns*. Review of Financial Studies, 29(1), 5–68; White, H. (2000). *A reality check for data snooping*. Econometrica, 68(5), 1097–1126.

### 4.3 Criterio para "pasa" (se tienen que cumplir todos)

1. **Retorno neto > 0 con significancia:** contraste unilateral sobre los **retornos mensuales** netos de la cartera en la validación (mensuales, para no inflar la muestra con la autocorrelación diaria), al nivel de su familia (1,25 % en la ronda 1). Tiene que pasar con **los dos** escenarios de financiación.
2. **Sharpe deflactado ≥ 0,95**, contando todas las pruebas registradas hasta ese momento (incluidas las de la estrategia actual y ML).
3. **Consistencia:** retorno neto medio positivo en las **dos mitades** de la validación (2013–2019 y 2020–2025) y en al menos **2 de los 3** índices.
4. **Costos estresados:** con costos × 1,5 el retorno neto sigue positivo (no hace falta que sea significativo).
5. **Contra comprar y mantener:** para A (que está expuesta al mercado la mayor parte del tiempo), Sharpe de la validación ≥ el de comprar y mantener el CFD en el mismo período, con sus costos.

**Qué se puede esperar de la potencia.** Con ~12,7 años de validación, pasar el punto 1 al 1,25 % unilateral pide, en retornos mensuales, un Sharpe anual de **≈ 0,63 o más**. Es una vara alta a propósito. C es la única con muestra para detectar efectos chicos por operación (~0,05 R con ~3000 operaciones por índice); A y D solo pasan si el efecto es grande.

### 4.4 Qué se hace con el resultado

- **Pasa:** se presenta al dueño. Él decide si se gasta el fuera de muestra en ese candidato congelado. Si el fuera de muestra no confirma (retorno neto ≤ 0), la familia se cierra.
- **No pasa:** la familia se cierra. **No** se prueban variantes de la misma familia sobre los mismos datos (otro rango de apertura, otro lookback). Eso sería otra familia, con su costo en el presupuesto.
- **No concluyente** (la muestra no alcanza, por ejemplo D2 si faltan fechas): se informa así, sin reintentar con cambios.
- Ningún "pasa" habilita a operar: falta la fase 5 completa, el papel (3 meses y 50 operaciones, spec §15) y, para dinero real, el ADR de las dos personas.

## 5. Orden recomendado y costo

| Orden | Familia | Por qué en este orden | Lo que hace falta antes | Cómputo |
|---|---|---|---|---|
| 1 | **C. Ruptura del rango de apertura** | Mejor costo/R (0,02–0,07), más muestra, intradía (sin financiación ni problema de la base), usa las velas de 15m que el motor ya maneja | Issue + pre-registro; módulo de estrategia nuevo en Rust (`engine/`, los cálculos de trading viven solo ahí) con tests; decidir el máximo de stop | Bajo: mismo backtest de 15m que hoy, una corrida por escenario |
| 2 | **D1. Cambio de mes** | Cero parámetros, literatura larga, costo ≈ 0,03 R; rápida de implementar | **Issue de la base del futuro en tenencias de varios días** (§2.3) y confirmar con IG el ajuste por dividendos; capital mínimo en US500 | Muy bajo: ~12 eventos por año |
| 3 | **A. Tendencia de baja frecuencia** | La evidencia académica más fuerte, pero cambia la spec (diario, salidas propias) y choca con el tamaño mínimo y la financiación | Lo de D1 + decisión del dueño sobre capital o riesgo por operación + cambio de spec | Bajo |
| 4 | **B. Momentum intradía** | Literatura fuerte, pero la vara de acierto (55–59 %) es alta; mirar sobre todo US100 | Issue + pre-registro | Bajo |
| 5 | **D2. Antes de la Fed** | Pocas observaciones; el efecto es de una muestra anterior | Cargar las fechas oficiales del FOMC desde 2011 | Muy bajo |
| 6 | **E. Reversión de corto plazo** | La fuente más débil y la regla cambia con el stop obligatorio | Lo de D1 | Bajo |

- **Ronda 1 = C y D1** (K = 2). Si ninguna pasa, la recomendación es parar y revisar el planteo con el dueño antes de gastar la ronda 2.
- El costo real **no es de cómputo** (los backtests de 15 años corren en el Mac mini como los de #421): es el desarrollo de cada estrategia con sus tests y, sobre todo, el **presupuesto de error**. Cada familia que se mira gasta una parte, y no se recupera.
- **Datos:** no hace falta cargar nada para C, B y D1. Para el DE40 en cualquier familia, más historia de FDAX (ya propuesto en #373). Para D2, las fechas del FOMC. Para el oro, confirmar valor por punto, tamaño mínimo y spread típico de IG.

## 6. Qué decide el dueño

1. Si se sigue este programa, y con qué familias en la ronda 1 (propuesta: C y D1).
2. El presupuesto de error total (propuesta: 5 % para todo el programa) y su reparto.
3. Para A, D y E: el capital del backtest o el riesgo por operación, por el tamaño mínimo de IG.
4. Abrir los Issues previos: base del futuro en tenencias de varios días, dividendos en los CFD de índices de IG y fechas del FOMC.
5. Cualquier cambio de spec (timeframe, salidas, ventana de horario) se aprueba aparte, familia por familia.

## Límites de esta propuesta

- Los costos en R son **estimaciones** con medianas de volatilidad de 2020–2025, no resultados de backtest. De 2011 a 2019 el índice estaba más bajo y el mismo spread en puntos pesa más (#461).
- Para medir las volatilidades se miraron los rangos de los futuros en el período de investigación. **No se miró el resultado de ninguna regla**, para que el pre-registro siga limpio.
- Las citas son de artículos publicados. Los dos libros de practicantes (Crabel; Connors y Alvarez) se marcan como tales. Los efectos publicados suelen achicarse después de publicarse (McLean y Pontiff, 2016).
- Ver también `docs/research/por-que-fallan-los-algoritmos.md` para el contexto general.
