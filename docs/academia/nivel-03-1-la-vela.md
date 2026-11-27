# 3.1 La vela (OHLC)

> **Nivel 3 · Velas y timeframes.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los
> ejemplos (6.000) son redondos, no cotizaciones.

## Objetivo

Leer una [vela](../GLOSARIO.md#vela) por sus cuatro precios (apertura, máximo, mínimo y cierre) y
saber por qué el motor solo usa la [vela cerrada](../GLOSARIO.md#vela-cerrada).

## La idea

Una vela resume todas las operaciones de un período (15 minutos, 1 hora, 1 día) en cuatro precios:

| Letra | Precio | Qué es |
|---|---|---|
| **O** (open) | Apertura | El primer precio del período. |
| **H** (high) | Máximo | El precio más alto que se operó. |
| **L** (low) | Mínimo | El precio más bajo que se operó. |
| **C** (close) | Cierre | El último precio del período. |

Si el cierre queda **arriba** de la apertura, la vela es **alcista** (se dibuja verde o hueca); si
queda **abajo**, es **bajista** (roja o llena). Con esos cuatro números el motor mide las partes de
la vela, siempre igual:

- **cuerpo** = |C − O|: cuánto se movió de la apertura al cierre;
- **rango** = H − L: todo el recorrido del período;
- **mecha inferior** = min(O, C) − L, y **mecha superior** = H − max(O, C): hasta dónde llegó el
  precio y volvió.

Una mecha larga dice "el precio fue hasta ahí y no se quedó". No dice por qué, ni qué va a pasar
después.

**Vela cerrada frente a vela en formación.** Mientras el período no terminó, la vela **cambia**: el
máximo puede subir, el mínimo bajar y el cierre moverse. Por eso el motor usa una vela recién cuando
cerró: una vela de 15 m que empieza a las 15:30 se usa desde las 15:45:00. Si usara la vela en
formación, un patrón podría aparecer y desaparecer (eso se llama
[repintado](../GLOSARIO.md#repintado)), y un backtest estaría mirando un dato que en vivo todavía
no existía.

**Una vela con nombre, como la define el motor.** El [martillo](../GLOSARIO.md#martillo) no es "una
vela que parece un martillo": tiene reglas medibles (`config/patterns.toml`). Para un largo:

1. venía cayendo: el cierre de la vela anterior es menor que el de 5 velas antes;
2. mecha inferior ≥ **2 × cuerpo**;
3. mecha superior ≤ **0,3 × rango**;
4. cuerpo ≥ **0,05 × rango** (descarta velas sin cuerpo);
5. rango ≥ **0,5 × [ATR](../GLOSARIO.md#atr)** de 14 velas (descarta velas diminutas).

Su espejo para cortos es la [estrella fugaz](../GLOSARIO.md#estrella-fugaz). Que una vela cumpla las
reglas no quiere decir que el precio vaya a subir: es solo una definición objetiva para poder
medir, después, si sirve o no.

> **Probá antes de seguir:** a las 15:40 ves una vela de 15 m con una mecha inferior enorme. ¿Ya es
> un martillo?
>
> <details><summary>Ver la respuesta</summary>
>
> Todavía no se sabe: la vela cierra a las 15:45 y hasta entonces el cierre puede caer y borrar la
> mecha. El motor la mira recién a las 15:45:00.
>
> </details>

## Interactivo

La vela en formación de IBKR (`ibkr_live_bar.v1`) al lado de la última vela cerrada: se ve cómo el
máximo, el mínimo y el cierre de la que está abierta se mueven, y cómo la cerrada ya no cambia. La
vela en formación es **solo visual**: ninguna señal la usa. Sin suscripción a precios en vivo, IBKR
la manda con unos 10 minutos de atraso.

## Ejemplo resuelto

Vela de 15 m de US500: **O 6.000, H 6.012, L 5.996, C 6.010**.

- Cerró arriba de la apertura: **alcista**.
- Cuerpo = 6.010 − 6.000 = **10 puntos**. Rango = 6.012 − 5.996 = **16 puntos**.
- Mecha superior = 6.012 − 6.010 = **2**. Mecha inferior = 6.000 − 5.996 = **4**.

Otra vela, después de una caída, con un ATR de 20 puntos: **O 6.000, H 6.005, L 5.980, C 6.004**.
Cuerpo 4, rango 25, mecha inferior 20 (≥ 2 × 4 = 8), mecha superior 1 (≤ 0,3 × 25 = 7,5), cuerpo
4 ≥ 0,05 × 25 = 1,25 y rango 25 ≥ 0,5 × 20 = 10: cumple las reglas de forma del martillo.

## Ejercicio

Clasificá estas 5 velas (precios de ejemplo): ¿alcista o bajista?, ¿cuál tiene la mecha inferior
más larga?, ¿cuál casi no tiene cuerpo?

| Vela | O | H | L | C |
|---|---|---|---|---|
| A | 6.000 | 6.008 | 5.998 | 6.006 |
| B | 6.006 | 6.007 | 5.990 | 5.992 |
| C | 5.992 | 5.994 | 5.975 | 5.991 |
| D | 5.991 | 6.003 | 5.989 | 6.002 |
| E | 6.002 | 6.010 | 5.994 | 6.002 |

<details><summary>Ver la solución</summary>

A alcista (cuerpo 6). B bajista (cuerpo 14). C bajista (cuerpo 1, mecha inferior 16: la más
larga). D alcista (cuerpo 11). E sin cuerpo (abre y cierra en 6.002): rango 16, cuerpo 0, así que
no puede ser martillo (cuerpo < 0,05 × rango).

</details>

## Examen

7 preguntas (4 de lectura de velas y 3 de concepto). Para aprobar hace falta el 80 % (umbral
provisorio).

## Desbloquea

- **Gráfico de velas y OHLC bajo el cursor**: al pasar el cursor por una vela se ven sus cuatro
  precios.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Una vela de 15 m se usa recién cuando cerró | `Una vela de 15m que empieza a las 15:30 se usa recién desde las 15:45:00` | `docs/spec/fase-1-especificacion.md` §3 | A |
| Martillo: mecha del rechazo ≥ 2 × cuerpo | 2 (`2`) | `config/patterns.toml` `candles.min_wick_to_body` | A |
| Martillo: mecha contraria ≤ 0,3 × rango | 0,3 (`0.3`) | `config/patterns.toml` `candles.max_opposite_wick_to_range` | A |
| Martillo: cuerpo ≥ 0,05 × rango | 0,05 (`0.05`) | `config/patterns.toml` `candles.min_body_to_range` | A |
| Martillo: rango ≥ 0,5 × ATR | 0,5 (`0.5`) | `config/patterns.toml` `candles.min_range_to_atr` | A |
| ATR de las velas: 14 períodos | 14 (`14`) | `config/patterns.toml` `candles.atr_period` | A |
| Caída previa: cierre anterior contra el de 5 velas antes | 5 (`5`) | `config/patterns.toml` `candles.prior_move_lookback` | A |
| Partes de la vela (cuerpo, rango, mechas) | `mecha_inf = min(O,C) − L` | `docs/spec/fase-1-especificacion.md` §5.1 | A |
| La vela en formación de IBKR no la usa ninguna señal | `SOLO VISUAL` | `contracts/ibkr_live_bar.v1.schema.json` | A |
| Sin suscripción, unos 10 minutos de atraso | `delay_seconds ~600` | `contracts/ibkr_live_bar.v1.schema.json` | A |
| Cómo se forman los precios operados | — | [Harris, *Trading and Exchanges*, Oxford University Press (2002/2003)](https://ideas.repec.org/b/oxp/obooks/9780195144703.html) | B |
