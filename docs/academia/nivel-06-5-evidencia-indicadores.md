# 6.5 Evidencia: reglas técnicas y fórmulas verificadas

> **Nivel 6 · Indicadores.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del pasado: no
> dicen nada seguro del futuro.

## Objetivo

Separar dos preguntas que se suelen mezclar: **¿la fórmula está bien calculada?** (eso lo probamos
con tests) y **¿la regla sirve para ganar después de costos?** (eso lo dicen los estudios y nuestro
registro, y casi siempre la respuesta es no).

## La idea

**1. Cómo sabemos que nuestras fórmulas están bien.** El motor dice que su SMA, su EMA y su ATR son
iguales a los de TradingView. No se le cree: se prueba.

- **Contra la fórmula publicada de TradingView** (`ta.sma`, `ta.ema`, `ta.atr`), calculada fuera
  del motor en aritmética exacta sobre **300 velas sintéticas**: tienen que coincidir **desde la
  primera vela**, con una tolerancia relativa de 10⁻¹⁸. Corre en el CI en cada cambio.
- **Contra TA-Lib 0.8.3**, otra implementación independiente que calcula en `float`: tolerancia de
  10⁻¹². El ATR de TA-Lib empieza una vela más tarde, así que se compara cuando esa diferencia
  inicial ya se borró.
- **Contra una exportación real de TradingView**: el test existe, pero corre **a mano**, porque la
  exportación tiene precios de un proveedor y no se publica. Todavía no hay una corrida anotada en el
  Issue #666. Hasta entonces, la prueba es contra la fórmula publicada, no contra la pantalla de
  TradingView.
- **Sin repintado:** hay tests que agregan velas nuevas y verifican que el valor de una vela cerrada
  no cambia nunca.

**Que la fórmula esté bien no dice nada de si la regla sirve.** Para eso hay que mirar la evidencia.

**2. Lo que dicen los estudios.**

- **Brock, Lakonishok y LeBaron (1992):** probaron reglas de medias móviles y de ruptura de rango en
  el índice Dow Jones de **1897 a 1986**. Las señales de compra rindieron más que las de venta, y
  los resultados no encajaban con cuatro modelos de "precio al azar". Parecía que las reglas
  simples funcionaban.
- **Sullivan, Timmermann y White (1999):** repitieron el estudio, pero con **7.846 reglas** y
  corrigiendo por haber probado tantas (el "Reality Check" de White). En 1897–1986 algunas reglas
  seguían ganándole a la referencia aun con la corrección. Pero en los **10 años siguientes
  (1987–1996)**, fuera de muestra, la evidencia casi desapareció: la probabilidad de que la mejor
  regla **no** le ganara a la referencia fue de casi el 12 %, y la mejor regla elegida a fines de 1986
  tuvo un rendimiento medio de 2,8 % con un p de 0,322 (nada distinguible del azar).
- **Marshall, Young y Rose (2006):** señales de velas japonesas en las acciones del Dow Jones de
  **1992 a 2002**: **ninguna evidencia** de que alguien que opera con velas le gane al mercado.
- **Park e Irwin (2007):** revisaron **95 estudios modernos**: 56 positivos, 20 negativos y 19
  mixtos. Pero la mayoría tiene problemas: probar muchas reglas sobre los mismos datos, elegir la
  regla después de ver el resultado, y estimar mal el riesgo y los costos.

**3. Lo que dio nuestro registro.**

| Prueba | Resultado | Fila |
|---|---|---|
| Tendencia de 12 meses (una regla técnica simple) | +0,080 % por mes, p 0,14: **no pasó** | R016 |
| Exposición manejada por volatilidad | alfa +0,19 % por mes, p 0,17: **no pasó** | R019 |
| Cruces de EMA en 1 h (laboratorio, exploratorio) | +0,068 a +0,115 R, pero PBO 0,81: probable suerte | R040 |

**Cómo se leen juntas.** Las reglas técnicas simples parecían funcionar hasta que se corrigió por
la cantidad de pruebas y se miró fuera de muestra. Por eso, en esta Academia, un indicador es una
**medida** (cuánto se mueve el precio, dónde está el promedio), no una señal: si querés usarlo para
operar, hay que escribir la regla, pre-registrarla y probarla con costos (niveles 9 y 10).

> **Probá antes de seguir:** si probás 7.846 reglas sobre los mismos 90 años, ¿por qué es
> esperable que alguna "gane" aunque ninguna tenga ventaja?
>
> <details><summary>Ver la respuesta</summary>
>
> Por azar: con tantas pruebas, algunas salen bien por suerte. Sin corregir por la cantidad de
> pruebas, la mejor parece una ventaja. Por eso Sullivan, Timmermann y White usan el Reality Check
> y nosotros el Sharpe deflactado y el PBO (nivel 9).
>
> </details>

## Interactivo

Las filas R016, R019 y R040 del registro de resultados con su informe, y el test de referencia de
indicadores (`engine/crates/indicators/tests/reference.rs`) con lo que compara.

## Ejercicio

Escribí en dos frases la diferencia entre "el ATR del motor coincide con el de TradingView" y "una
regla con ATR gana dinero".

<details><summary>Ver la solución</summary>

La primera es una afirmación sobre el **cálculo**: se prueba con la fórmula publicada y con otra
implementación, y la probamos. La segunda es una afirmación sobre el **mercado**: se prueba con una
hipótesis pre-registrada, datos fuera de muestra y costos, y casi nunca pasa.

</details>

## Examen

Esta lección no tiene examen: es la evidencia del nivel. Se completa al leerla.

## Desbloquea

Nada nuevo: las capas de indicadores se abren en 6.1, 6.2 y 6.4.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Contraste con la fórmula publicada, desde la primera vela | `matches_the_published_tradingview_formulas_from_the_first_candle` | `engine/crates/indicators/tests/reference.rs` | A |
| 300 velas sintéticas | `300 velas **sintéticas**` | `engine/crates/indicators/tests/reference.rs` | A |
| Tolerancia contra la fórmula publicada | `let tol = Decimal::new(1, 18);` | `engine/crates/indicators/tests/reference.rs` | A |
| Versión de TA-Lib | `TA-Lib 0.8.3` | `engine/crates/indicators/tests/reference.rs` | A |
| Tolerancia contra TA-Lib | `let tol = Decimal::new(1, 12);` | `engine/crates/indicators/tests/reference.rs` | A |
| La exportación real de TradingView corre a mano | `solo corre a mano` | `engine/crates/indicators/tests/reference.rs` | A |
| La exportación no se commitea | `el CSV no se commitea nunca` | `docs/guides/golden-tradingview.md` | A |
| Sin repintado (test) | `values_never_change_when_new_candles_arrive` | `engine/crates/indicators/src/moving_average.rs` | A |
| Tendencia de 12 meses | `+0.080 % (p 0.14; 0.087 vs 0.261)` | `docs/analysis/registro-de-resultados.csv` `R016` | A |
| Exposición manejada por volatilidad | `+0.192 % (p 0.171; 0.245 vs 0.261)` | `docs/analysis/registro-de-resultados.csv` `R019` | A |
| Cruces de EMA en el laboratorio | `+0.068 a +0.115 R; DSR 0.019 a 0.144; PBO 0.81` | `docs/analysis/registro-de-resultados.csv` `R040` | A |
| Medias y ruptura de rango, Dow Jones 1897–1986; compra mejor que venta | — | [Brock, Lakonishok y LeBaron (1992), *Simple Technical Trading Rules and the Stochastic Properties of Stock Returns*, Journal of Finance 47(5)](https://ideas.repec.org/a/bla/jfinan/v47y1992i5p1731-64.html), resumen leído el 26-11-2026 | A |
| 7.846 reglas; fuera de muestra 1987–1996: casi 12 %; mejor regla 2,8 % con p 0,322 | — | [Sullivan, Timmermann y White (1999), *Data-Snooping, Technical Trading Rule Performance, and the Bootstrap*, Journal of Finance 54(5)](https://ideas.repec.org/a/bla/jfinan/v54y1999i5p1647-1691.html), texto completo leído el 26-11-2026 | A |
| Velas japonesas en el Dow Jones 1992–2002: sin valor | — | [Marshall, Young y Rose (2006), *Candlestick technical trading strategies: Can they create value for investors?*, Journal of Banking & Finance 30(8)](https://ideas.repec.org/a/eee/jbfina/v30y2006i8p2303-2323.html); el contenido, verificado con la tesis de la fila siguiente | A |
| Lo mismo en la tesis del primer autor (resumen) | — | [Marshall (2005), tesis doctoral, Massey University](https://mro.massey.ac.nz/handle/10179/1604), resumen leído el 26-11-2026 | B |
| 95 estudios: 56 positivos, 20 negativos, 19 mixtos; problemas de método | — | [Park e Irwin (2007), *What Do We Know About the Profitability of Technical Analysis?*, Journal of Economic Surveys 21(4)](https://ideas.repec.org/a/bla/jecsur/v21y2007i4p786-826.html), resumen leído el 26-11-2026 | A |
