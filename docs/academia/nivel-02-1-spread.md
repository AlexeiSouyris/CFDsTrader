# 2.1 Spread

> **Nivel 2 · Costos reales.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Saber que **se entra pagando**: cuánto es nuestro [spread](../GLOSARIO.md#spread) típico en cada
instrumento y cuánto pesa frente a tu stop, medido en [R](../GLOSARIO.md#r-multiplo).

## La idea

El spread es la diferencia entre el precio de compra (BUY) y el de venta (SELL). Si comprás y
vendés en el mismo instante, sin que el mercado se mueva, **perdés el spread entero**. El motor lo
cobra así: medio spread al entrar y medio al salir, siempre en contra.

Nuestros spreads típicos (el mayor entre el medido en la demo de IG en la sesión regular y el
mínimo que IG publica para la cuenta real, para no subestimar el costo):

| Instrumento | Spread típico | Mínimo publicado por IG |
|---|---|---|
| US500 | 0,9 puntos | 0,8 |
| US100 | 1,6 puntos | 1,6 |
| US30 | 3,7 puntos | 3,4 |
| DE40 | 1,8 puntos | 1,8 |
| Oro (XAUUSD) | sin medir todavía | 0,9 |

**El spread cambia con la hora.** IG usa spreads fijos por franja. Fuera de la sesión regular
fueron entre 1,5 y 5 veces mayores en la demo: entre las 21 y las 22 UTC, US30 pasó de 3,7 a 12,2
puntos, US100 de 1,5 a 5,8 y DE40 de 1,4 a 7,0.

**Medirlo en R.** Como el stop define 1 R (lo que perdés si toca el stop), el spread se puede
expresar en R: **spread ÷ distancia del stop**. Cuanto más corto el stop, más pesa el mismo spread.

> **Probá antes de seguir:** con el mismo spread, ¿qué pesa más en R: un stop de 4 puntos o uno de
> 8?
>
> <details><summary>Ver la respuesta</summary>
>
> El de 4: el mismo spread es el doble de grande frente a un stop la mitad de largo.
>
> </details>

## Interactivo

El spread dibujado sobre una vela real de 15 minutos: una franja muestra cuánto de la vela "es
costo". El motor manda la vela y el spread del catálogo.

## Ejemplo resuelto

US500. Nuestra estrategia de 15 minutos usa stops de 1 a 1,5 ATR de 15 minutos, unos 8 a 10 puntos
en US500. Tomemos un stop de 1 ATR = 8 puntos, así que **1 R = 8 puntos**.

- Spread en R: 0,9 ÷ 8 = **0,1125 R**. Más de una décima de R antes de que el precio se mueva.
- Con un stop de 4 puntos: 0,9 ÷ 4 = **0,225 R**.
- Con tamaño 2 € por punto en US30, el spread de la operación completa es 3,7 × 2 = **7,40 €**.

Si una idea gana en promedio 0,1 R por operación antes de costos, el spread solo ya se la come.

## Ejercicio

Expresá el spread en R para un stop de 1 ATR en los cuatro índices, con estos ATR de 15 minutos de
ejemplo: US500 8 puntos, US100 30, US30 40 y DE40 20.

<details><summary>Ver la solución</summary>

US500 0,9 ÷ 8 ≈ 0,11 R · US100 1,6 ÷ 30 ≈ 0,05 R · US30 3,7 ÷ 40 ≈ 0,09 R · DE40 1,8 ÷ 20 = 0,09 R.
Los ATR son de ejemplo; en el interactivo el motor usa el ATR real de la vela.

</details>

## Examen

5 preguntas de cálculo y lectura. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Columna de costos** del ticket.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Spread típico US500 | 0,9 (`0.9`) | `config/instruments.toml` `instruments.US500.spread_typical_points` (medido en la demo, #67) | A |
| Spread típico US100 | 1,6 (`1.6`) | `config/instruments.toml` `instruments.US100.spread_typical_points` (publicado por IG) | A |
| Spread típico US30 | 3,7 (`3.7`) | `config/instruments.toml` `instruments.US30.spread_typical_points` (medido en la demo) | A |
| Spread típico DE40 | 1,8 (`1.8`) | `config/instruments.toml` `instruments.DE40.spread_typical_points` (publicado por IG) | A |
| Spread mínimo publicado US500 | 0,8 (`0.8`) | `config/instruments.toml` `instruments.US500.spread_min_points` | A |
| Spread mínimo publicado US30 | 3,4 (`3.4`) | `config/instruments.toml` `instruments.US30.spread_min_points` | A |
| Spread mínimo publicado del oro | 0,9 (`0.9`) | `config/instruments.toml` `instruments.XAUUSD.spread_min_points` | A |
| Spreads fuera de sesión (21–22 UTC): US30, US100, DE40 | 12,2 (`12,2`) | `docs/research/ig-spreads-2026-11.md` (mediana por hora, demo, 09 al 20-11-2026) | A |
| Fuera de sesión, entre 1,5 y 5 veces mayor | `entre 1,5 y 5 veces mayor` | `docs/research/ig-spreads-2026-11.md` | A |
| IG usa spreads fijos por franja | `spreads fijos por franja` | `docs/research/ig-spreads-2026-11.md` | A |
| Stops de la estrategia: unos 8 a 10 puntos en US500 | `unos 8–10 puntos en US500` | `docs/analysis/2026-11-propuesta-investigacion.md` | A |
| Medio spread al entrar y medio al salir | `medio ± (spread / 2 + slippage)` | `engine/crates/backtest/src/costs.rs` | A |
| Spreads de CFD de IG (Wall Street 3,4, Alemania 40 1,8) | — | [IG España, "Nuestros costes"](https://www.ig.com/es/nuestros-costes), leído el 26-11-2026 | A |
| Costos de transacción y el spread | — | [Harris, *Trading and Exchanges*, Oxford University Press (2002/2003)](https://ideas.repec.org/b/oxp/obooks/9780195144703.html) | B |
