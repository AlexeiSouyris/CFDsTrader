# 7.1 R

> **Nivel 7 · Riesgo y tamaño.** Educativo: no es consejo de inversión ni promete ganancias. Todo
> pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos
> son redondos, no cotizaciones.

## Objetivo

Medir todo en múltiplos del riesgo: **1 R = lo que perdés si toca el stop**. Pasar de puntos a
euros y a R sin equivocarte.

## La idea

Ganar 300 € no dice nada si no sabés cuánto arriesgaste. ¿300 € arriesgando 100? ¿O arriesgando
3.000? Por eso medimos en [R](../GLOSARIO.md#r-multiplo):

- **1 R** = tamaño × distancia al [stop](../GLOSARIO.md#stop-loss). Con 2 € por punto y el stop a 25
  puntos, 1 R = 50 €.
- **Resultado en R** = resultado en € ÷ 1 R. Ganar 120 € con 1 R = 50 € es **+2,4 R**; perder 30 €
  es **−0,6 R**.
- **Objetivo en R:** un objetivo a 2 R está al doble de la distancia del stop.

Como el [riesgo por operación](../GLOSARIO.md#riesgo-por-operacion) del motor es el **1 %** del
capital, con 10.000 € de capital 1 R son 100 €: ganar 250 € es +2,5 R.

**Los costos también se miden en R.** El spread típico del US500 es 0,9 puntos. Con el stop a 30
puntos, pagar el spread cuesta 0,9 ÷ 30 = **0,03 R** por operación. Con un stop más corto, el mismo
spread pesa más. En la estrategia en uso, los costos se comían **0,18 R por operación** en US500
(R001, nivel 2).

**Un detalle del motor: qué entra en 1 R.**

- En el **examen de replay**, 1 R = la distancia del cierre de la vela de la decisión al stop.
- En el **backtest de investigación**, 1 R es el riesgo que calcula el módulo de riesgo, que suma
  el colchón por gaps de 0,25 ATR (lección 7.3). Por eso ahí 1 R es un poco más grande que la sola
  distancia al stop, y los R por operación salen un poco más chicos.

Cuando compares resultados en R, fijate de dónde salen.

> **Probá antes de seguir:** tamaño 3 € por punto, stop a 20 puntos. La operación cierra con +50
> puntos. ¿Cuántos R ganó?
>
> <details><summary>Ver la respuesta</summary>
>
> 1 R = 3 × 20 = 60 €. Ganó 3 × 50 = 150 €. 150 ÷ 60 = **+2,5 R**. Más corto: 50 puntos ÷ 20
> puntos de stop = 2,5 R (el tamaño se cancela).
>
> </details>

## Interactivo

Un conversor puntos ↔ € ↔ R: elegís tamaño, distancia al stop y resultado, y el motor devuelve las
tres medidas.

## Ejercicio

Cinco conversiones, todas con tamaño 2 € por punto y stop a 40 puntos (1 R = 80 €):

1. +100 puntos → ¿R?
2. −40 puntos → ¿R?
3. +200 € → ¿R?
4. Objetivo a 1,5 R → ¿a cuántos puntos de la entrada?
5. Spread de 1,6 puntos → ¿cuántos R?

<details><summary>Ver la solución</summary>

1. 100 ÷ 40 = +2,5 R.
2. −40 ÷ 40 = −1 R (tocó el stop justo, sin gap).
3. 200 ÷ 80 = +2,5 R.
4. 1,5 × 40 = 60 puntos.
5. 1,6 ÷ 40 = 0,04 R.

</details>

## Examen

6 cálculos. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada todavía: el ticket de paper se abre en 7.3.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Riesgo por operación | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
| Spread típico del US500 | 0,9 puntos (`0.9`) | `config/instruments.toml` `instruments.US500.spread_typical_points` | A |
| Spread típico del US100 | 1,6 puntos (`1.6`) | `config/instruments.toml` `instruments.US100.spread_typical_points` | A |
| Costos de la estrategia en uso en US500 | 0,18 R (`-0.19 (-0.02 / 0.18)`) | `docs/analysis/registro-de-resultados.csv` `R001` | A |
| Examen de replay: 1 R = distancia del cierre de t al stop | `R = distancia del cierre de t al stop` | `engine/crates/academy/src/grade.rs` | A |
| Backtest de investigación: 1 R incluye el colchón por gaps | `el backtest de investigación usa` | `docs/analysis/2026-11-riesgo-propuesta.md` | A |
| Colchón por gaps | 0,25 ATR (`0.25`) | `config/risk.toml` `gap.gap_atr` | A |
