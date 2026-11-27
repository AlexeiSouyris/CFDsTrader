# 4.1 Pivote confirmado

> **Nivel 4 · Estructura.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Saber que un máximo o mínimo local ([pivote](../GLOSARIO.md#pivote)) existe **recién después de N
velas de confirmación**, y marcar pivotes como los marca el motor.

## La idea

Mirando un gráfico terminado, los picos y los valles "saltan a la vista". En vivo no: el pico de
recién puede ser superado por la vela siguiente. Por eso el motor usa una regla medible:

- Una vela `i` es **pivote alto** si su máximo es **estrictamente mayor** que el de las **3 velas
  anteriores** y el de las **3 posteriores** (N = 3, `config/patterns.toml`).
- El **pivote bajo** es el espejo, con los mínimos.
- El pivote se **confirma en la vela `i + 3`**, al cerrar. Antes de eso **no existe** para el
  sistema: es una suposición.

Dos detalles que importan:

1. **Estrictamente.** Si dos velas tienen el mismo máximo, ninguna es pivote alto: "igual" no es
   "mayor".
2. **Una vela posterior más alta lo cancela.** Si en las 3 velas siguientes aparece un máximo más
   alto, el candidato deja de serlo. Por eso hay que esperar.

**Sin repintado.** Una vez confirmado, un pivote no cambia nunca al llegar velas nuevas. Y no se usa
antes de su confirmación: ni en los patrones, ni en los niveles, ni en el replay.

**En 15 m, ¿cuánto tarda?** Un pivote en la vela de las 10:00 se confirma con la vela de las 10:45,
que cierra a las 11:00: **una hora después** de que empezó la vela del pivote.

> **Probá antes de seguir:** los máximos de 7 velas son 6.010, 6.014, 6.018, **6.025**, 6.020,
> 6.016 y 6.025. ¿La cuarta vela es pivote alto?
>
> <details><summary>Ver la respuesta</summary>
>
> No: la séptima vela tiene el mismo máximo (6.025), y la regla pide **estrictamente mayor**. Si la
> séptima hubiera llegado a 6.019, la cuarta sería pivote alto, confirmado al cierre de la séptima.
>
> </details>

## Interactivo

Un replay donde el pivote "aparece" recién en la vela N: avanzás vela por vela y la capa de pivotes
del motor marca cada uno en el momento en que se confirma, no antes.

## Ejemplo resuelto

Mínimos de 9 velas de 15 m: 5.990, 5.985, 5.982, **5.975**, 5.980, 5.983, 5.979, 5.986, 5.990.

- La vela 4 (5.975) es menor que las 3 anteriores (5.990, 5.985, 5.982) y que las 3 posteriores
  (5.980, 5.983, 5.979): **pivote bajo**.
- Se confirma al cierre de la vela 7. Hasta ese cierre, la vela 7 podía bajar de 5.975 y cancelarlo.
- La vela 7 (5.979) no es pivote: la vela 4, tres lugares antes, es más baja.

## Ejercicio

Máximos de 8 velas: 6.040, 6.046, 6.051, 6.049, 6.044, 6.047, 6.043, 6.041. ¿Hay algún pivote
alto? ¿En qué vela se confirma?

<details><summary>Ver la solución</summary>

La vela 3 (6.051) es mayor que las 2 anteriores, pero solo hay 2 velas antes de ella: le falta una
para tener 3 de cada lado, así que con N = 3 **no se puede evaluar**. Ninguna otra vela supera a
sus 3 vecinas de cada lado (la vela 6, 6.047, tiene a la vela 3 más alta tres lugares antes). Con
estas 8 velas no hay pivote alto confirmado.

</details>

## Examen

**Examen de replay de pivotes.** El motor elige un gráfico real con la fecha oculta (300 velas de
calentamiento y 400 de examen) y vos marcás **al menos 5 pivotes** mientras avanzás. Cada marca se
compara con los pivotes del motor (N = 3), calculados con las velas que viste:

- **acierto**: la misma vela y el mismo tipo (alto o bajo), marcado con el cursor en la vela de
  confirmación o después;
- **adelantada**: la vela era un pivote, pero la marcaste antes de que se confirmara (era una
  suposición): no cuenta como acierto;
- **error**: el motor no tiene un pivote de ese tipo en esa vela.

Aprobás con al menos 5 pivotes marcados y una precisión (aciertos ÷ marcas) de al menos el 80 %.
Los umbrales son **provisorios** hasta que el dueño los apruebe.

## Desbloquea

- **Capa de pivotes del motor**: los pivotes confirmados sobre el gráfico.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Velas de cada lado (N) | 3 (`3`) | `config/patterns.toml` `pivots.strength` | A |
| Regla: estrictamente mayor que las 3 anteriores y las 3 posteriores | `que el de las 3 velas anteriores y las 3 posteriores (N = 3)` | `docs/spec/fase-1-especificacion.md` §4 | A |
| Antes de confirmarse, no existe | `antes no existe para el sistema` | `docs/spec/fase-1-especificacion.md` §4 | A |
| Máximos iguales no son pivote | `equal_highs_are_not_a_pivot` | `engine/crates/patterns/src/pivots.rs` (test) | A |
| Una vela posterior más alta cancela el pivote | `a_later_higher_candle_cancels_the_pivot` | `engine/crates/patterns/src/pivots.rs` (test) | A |
| Un pivote no cambia al llegar velas nuevas | `pivots_never_change_when_new_candles_arrive` | `engine/crates/patterns/src/pivots.rs` (test) | A |
| Pivotes marcados como mínimo en el examen (provisorio) | 5 (`5`) | `config/academy.toml` `replay.pivots_per_exam` | A |
| Precisión para aprobar (provisorio, el mismo umbral que las preguntas) | 80 % (`80`) | `config/academy.toml` `thresholds.quiz_pass_pct` | A |
| Velas del examen después del calentamiento | 400 (`400`) | `config/academy.toml` `replay.exam_bars` | A |
| Marca antes de la confirmación: «adelantada» | `Early` | `engine/crates/academy/src/grade.rs` | A |
| Definir los patrones con un algoritmo quita la subjetividad | — | [Lo, Mamaysky y Wang (2000), *Foundations of Technical Analysis*, Journal of Finance 55(4) (versión NBER w7613)](https://www.nber.org/papers/w7613), resumen leído el 26-11-2026 | A |
