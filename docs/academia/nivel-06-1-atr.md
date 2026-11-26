# 6.1 ATR

> **Nivel 6 · Indicadores.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Medir el tamaño "normal" del movimiento con el [ATR](../GLOSARIO.md#atr) de Wilder, calcularlo a
mano como lo calcula el motor y usarlo para poner un stop a una distancia que no sea puro ruido.

## La idea

**El True Range (TR)** es lo que se movió una vela, contando el salto desde el cierre anterior. Es
el mayor de estos tres números:

1. máximo − mínimo de la vela;
2. la distancia entre el máximo y el cierre anterior;
3. la distancia entre el mínimo y el cierre anterior.

En la primera vela no hay cierre anterior: el TR es solo máximo − mínimo. Los puntos 2 y 3 existen
por los [gaps](../GLOSARIO.md#gap): una vela puede ser chiquita y aun así haber saltado mucho
desde el cierre de antes.

**El ATR(14) de Wilder** es un promedio de los TR con memoria:

- **Primer valor:** el promedio simple de los primeros 14 TR. Sale en la vela 14; antes no hay ATR
  (es el **calentamiento**).
- **Después:** ATR nuevo = (ATR anterior × 13 + TR de la vela) ÷ 14. Cada vela nueva pesa 1/14 y
  lo anterior conserva 13/14: por eso el ATR cambia despacio.

El período 14 es el de Wilder (1978), y es el que usa el motor: en las velas diarias (la R de los
análogos) y en las de 15 m y 1 h de los patrones y del examen de replay.

**Solo velas cerradas y sin repintado.** El ATR de una vela usa esa vela y las anteriores, nunca
la que está en formación. Una vez calculado, no cambia al llegar velas nuevas.

**Está en puntos, no en %.** Un ATR de 20 en un índice que cotiza a 6.000 es un 0,33 % del precio;
un ATR de 40 a 25.000 es un 0,16 %. Para comparar instrumentos, mirá el ATR frente al precio.

**Para qué lo usa el motor.** Casi todo lo que tiene "distancia" se mide en ATR:

- la distancia al stop tiene que estar **entre 0,5 y 3 ATR** (nivel 7);
- la estrategia en uso pone el stop **0,5 ATR** más allá de la estructura;
- el tamaño suma un colchón por gaps de **0,25 ATR** (nivel 7);
- en el examen de niveles, una marca acierta si queda a **0,25 ATR** de un nivel del motor.

**¿Coincide con TradingView?** El motor dice que su ATR es igual al `ta.atr` de TradingView, y lo
prueba: un test compara el ATR del motor con la fórmula publicada de Pine Script, en aritmética
exacta, sobre 300 velas sintéticas, **desde la primera vela** y con una tolerancia relativa de
10⁻¹⁸. También se compara con TA-Lib, que da su primer ATR una vela más tarde. La comparación con
una exportación real de TradingView solo se puede correr a mano (la exportación no se publica) y
todavía no hay una corrida anotada en el Issue #666. Más detalle en la lección 6.5.

> **Probá antes de seguir:** el cierre anterior fue 6.000. La vela siguiente abre con un gap y
> hace máximo 6.035 y mínimo 6.018. ¿Cuál es su TR?
>
> <details><summary>Ver la respuesta</summary>
>
> 35 puntos. Máximo − mínimo da 17; máximo − cierre anterior da 35; mínimo − cierre anterior da 18.
> El mayor es 35. Sin contar el gap, la vela "parecía" de 17.
>
> </details>

## Interactivo

El ATR(14) del motor sobre el gráfico, con un deslizador de período: cada cambio lo recalcula el
motor (la UI no calcula indicadores).

## Ejemplo resuelto

El ATR anterior es 20 y la vela nueva tiene un TR de 34, mucho más grande de lo normal.

ATR nuevo = (20 × 13 + 34) ÷ 14 = 294 ÷ 14 = **21**.

Una sola vela enorme sube el ATR apenas 1 punto. Si las velas grandes siguen, el ATR sigue
subiendo de a poco: mide el ruido de las últimas semanas de velas, no el de la última.

## Ejercicio

Poné el stop a **1,5 ATR** de la entrada en estos tres casos:

1. Largo en US500 a 6.000 con ATR 20.
2. Corto en US100 a 25.000 con ATR 40.
3. Largo en DE40 a 24.000 con ATR 30.

<details><summary>Ver la solución</summary>

1. 1,5 × 20 = 30 puntos: stop en **5.970**.
2. 1,5 × 40 = 60 puntos: en un corto el stop va arriba, en **25.060**.
3. 1,5 × 30 = 45 puntos: stop en **23.955**.

Los tres están dentro del rango permitido (0,5 a 3 ATR).

</details>

## Examen

6 preguntas (cálculos y conceptos). Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **ATR**: el ATR(14) del motor sobre el gráfico, con su período.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Período del ATR en velas diarias | 14 (`14`) | `config/indicators.toml` `daily.atr_period` | A |
| Período del ATR de los patrones y del examen de replay | 14 (`14`) | `config/patterns.toml` `candles.atr_period` | A |
| TR de la primera vela | `TR_0 = H_0 − L_0` | `engine/crates/indicators/src/atr.rs` | A |
| Suavizado de Wilder | `ATR_t = (ATR_{t−1} · (n − 1) + TR_t) / n` | `engine/crates/indicators/src/atr.rs` | A |
| Calentamiento: sin valor las primeras n − 1 velas | `Calentamiento: las primeras` | `engine/crates/indicators/src/atr.rs` | A |
| Stop mínimo, en ATR | 0,5 (`0.5`) | `config/risk.toml` `stop_distance.min_atr` | A |
| Stop máximo, en ATR | 3 (`3`) | `config/risk.toml` `stop_distance.max_atr` | A |
| Stop de la estrategia más allá de la estructura | 0,5 ATR (`0.5`) | `config/strategy.toml` `stop.atr_buffer` | A |
| Colchón por gaps | 0,25 ATR (`0.25`) | `config/risk.toml` `gap.gap_atr` | A |
| Tolerancia del examen de niveles | 0,25 ATR (`0.25`) | `config/academy.toml` `replay.level_tolerance_atr` | A |
| Contraste con la fórmula publicada de TradingView, desde la primera vela | `matches_the_published_tradingview_formulas_from_the_first_candle` | `engine/crates/indicators/tests/reference.rs` | A |
| 300 velas sintéticas | `300 velas **sintéticas**` | `engine/crates/indicators/tests/reference.rs` | A |
| Tolerancia relativa 10⁻¹⁸ | `let tol = Decimal::new(1, 18);` | `engine/crates/indicators/tests/reference.rs` | A |
| TA-Lib da su primer ATR una vela después | `el ATR de TA-Lib` | `engine/crates/indicators/tests/reference.rs` | A |
| La comparación con TradingView real se corre a mano | `solo corre a mano` | `engine/crates/indicators/tests/reference.rs` | A |
| ATR y RSI: de Wilder (1978), *New Concepts in Technical Trading Systems* | — | [Macroption, ficha del libro de Wilder (1978)](https://www.macroption.com/new-concepts-in-technical-trading-systems/), leída el 26-11-2026 | B |
