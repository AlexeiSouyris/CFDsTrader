# 4.3 Tendencia y ruptura

> **Nivel 4 · Estructura.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Reconocer una [tendencia](../GLOSARIO.md#tendencia) con pivotes confirmados, saber cómo la mide el
motor en 1 h, y leer una [ruptura](../GLOSARIO.md#ruptura) y un
[doble techo o doble suelo](../GLOSARIO.md#doble-techo) con su regla medible.

## La idea

**Tendencia con pivotes.** La idea clásica: **alcista** = máximos y mínimos cada vez más altos;
**bajista** = cada vez más bajos; si no, **rango**. Con pivotes confirmados (lección 4.1) se puede
comprobar sin opinar.

**La tendencia de 1 h del motor.** El motor no cuenta pivotes para la tendencia: usa una regla más
simple, sobre la **última vela de 1 h cerrada**:

```text
pendiente = SMA50_1h[ahora] − SMA50_1h[hace 5 velas]
cierre_1h > SMA50_1h y pendiente > 0  → ALCISTA
cierre_1h < SMA50_1h y pendiente < 0  → BAJISTA
si no                                 → NEUTRAL
```

En el puntaje de la estrategia en uso, operar **a favor** suma 2 y **en contra** resta 3. Es una
definición, no una promesa: la estrategia en uso, con esta regla, igual pierde después de costos
(nivel 2, R001–R003).

**Ruptura y retesteo.** Una ruptura es una vela que **cierra** del otro lado de un nivel (que el
máximo lo pase no alcanza). El [retesteo](../GLOSARIO.md#retesteo) es volver a tocar el nivel roto;
se enseña como descripción, pero el motor **no lo usa** como señal.

**Doble suelo, como lo define el motor** (spec §5.2; el doble techo es el espejo):

1. S1 y S2 son dos pivotes bajos confirmados de 15 m, S2 después de S1;
2. entre los dos hay de **10 a 60 velas**;
3. la diferencia |S1 − S2| ÷ S1 es de **0,3 % o menos**;
4. el [neckline](../GLOSARIO.md#neckline) es el máximo más alto entre S1 y S2, y la altura (neckline
   − el suelo más bajo) es de **al menos 1 ATR**;
5. **gatillo**: una vela cierra arriba del neckline con la anterior cerrada en el neckline o abajo,
   hasta **20 velas** después de confirmarse S2.

En el puntaje, esa ruptura suma 2. Otra vez: es una definición para poder medir, no una señal que
funcione por sí misma.

> **Probá antes de seguir:** un doble suelo con S1 = 6.000, S2 = 6.015 y 30 velas entre los dos.
> ¿Cumple la tolerancia?
>
> <details><summary>Ver la respuesta</summary>
>
> Sí: 15 ÷ 6.000 = 0,25 %, que es menos que 0,3 %. Falta comprobar la altura (neckline − 6.000 ≥ 1
> ATR) y esperar el gatillo.
>
> </details>

## Interactivo

La capa de tendencia de 1 h del motor sobre el gráfico de 15 m: cada tramo se pinta alcista,
bajista o neutral según la última vela de 1 h cerrada, y la capa de patrones marca los dobles
techos y suelos en la vela del gatillo.

## Ejemplo resuelto

Doble suelo en US500 con un ATR de 25 puntos: S1 = 6.000, S2 = 6.015, 30 velas entre los dos,
neckline 6.030.

- Velas entre los suelos: 30, entre 10 y 60. ✔
- Diferencia: 0,25 % ≤ 0,3 %. ✔
- Altura: 6.030 − 6.000 = 30 puntos = 1,2 ATR ≥ 1. ✔
- Gatillo: la vela que cierra en 6.034 con la anterior cerrada en 6.028 rompe el neckline. Si
  pasa dentro de las 20 velas después de confirmar S2, es el gatillo; si no, el patrón venció.

## Ejercicio

Clasificá estos tramos (pivotes confirmados, en orden) como alcista, bajista o rango:

1. Máximos 6.020 → 6.035 → 6.050; mínimos 5.990 → 6.005 → 6.015.
2. Máximos 6.050 → 6.040 → 6.025; mínimos 6.010 → 5.995 → 5.980.
3. Máximos 6.030 → 6.031 → 6.029; mínimos 5.990 → 5.992 → 5.989.
4. Máximos 6.020 → 6.040 → 6.035; mínimos 5.990 → 6.000 → 5.985.

<details><summary>Ver la solución</summary>

1 alcista. 2 bajista. 3 rango (máximos y mínimos casi iguales). 4 ninguno de los dos: un máximo
más bajo y un mínimo más bajo después de subir; podría ser el principio de un giro, pero con estos
pivotes **no** es una tendencia definida.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio). (El temario sumaba un replay;
el motor todavía no califica la clasificación de tendencias en un replay, así que por ahora son
solo preguntas.)

## Desbloquea

- **Capa de soportes y resistencias** del motor.
- **Patrones chartistas**: dobles techos y suelos marcados en su gatillo.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Media de la tendencia de 1 h | 50 velas (`50`) | `config/indicators.toml` `trend.sma_period` | A |
| Pendiente contra 5 velas antes | 5 (`5`) | `config/indicators.toml` `trend.slope_lookback` | A |
| Regla de la tendencia de 1 h | `pendiente = SMA50_1h[ahora] − SMA50_1h[hace 5 velas]` | `docs/spec/fase-1-especificacion.md` §4 | A |
| Puntaje con la tendencia a favor | +2 (`2`) | `config/strategy.toml` `score.trend_with` | A |
| Puntaje con la tendencia en contra | −3 (`-3`) | `config/strategy.toml` `score.trend_against` | A |
| Puntaje de la ruptura de un doble techo o suelo | +2 (`2`) | `config/strategy.toml` `score.double_break` | A |
| Velas entre los suelos: mínimo | 10 (`10`) | `config/patterns.toml` `double.min_bars_between` | A |
| Velas entre los suelos: máximo | 60 (`60`) | `config/patterns.toml` `double.max_bars_between` | A |
| Diferencia máxima entre suelos | 0,3 % (`0.3`) | `config/patterns.toml` `double.max_difference_pct` | A |
| Altura mínima | 1 ATR (`1`) | `config/patterns.toml` `double.min_height_atr` | A |
| El gatillo vale hasta estas velas después de S2 | 20 (`20`) | `config/patterns.toml` `double.max_bars_after_confirmation` | A |
| Gatillo: cierre arriba del neckline | `gatillo: C[t] > neckline y C[t−1] ≤ neckline` | `docs/spec/fase-1-especificacion.md` §5.2 | A |
| La estrategia en uso pierde después de costos | `-0.19 (-0.02 / 0.18)` | `docs/analysis/registro-de-resultados.csv` `R001` | A |
