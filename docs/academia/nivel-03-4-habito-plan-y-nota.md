# 3.4 Hábito: plan antes, nota después

> **Nivel 3 · Velas y timeframes.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Tomar el hábito que acompaña todo el curso desde acá: **escribir el plan antes** de cada ejercicio
de replay y **una nota después**, sin juzgarte por el resultado.

## La idea

**El plan, antes.** Antes de marcar una entrada, escribí en pocas líneas:

1. qué ves (por ejemplo: "tocó dos veces 6.000 y rebotó");
2. qué hacés y dónde ([stop](../GLOSARIO.md#stop-loss), objetivo, tamaño);
3. qué te diría que te equivocaste (el precio o la condición que invalida la idea).

En los exámenes de operaciones del replay, **una entrada sin plan escrito reprueba** esa regla del
proceso, aunque gane. El examen pide 10 operaciones, cada una con su plan: una sola no decide.

**La nota, después.** Al terminar, anotá: ¿seguiste el plan?, ¿qué pasó?, ¿qué harías distinto?
La pregunta es por el **proceso**, no por si ganaste. Una operación que siguió el plan y tocó el
stop es una buena operación con un mal resultado; una que ganó sin stop es una mala operación con
suerte.

**Por qué hace falta escribir.** Los estudios muestran sesgos que aparecen solos si no los
vigilás:

- Las pérdidas pesan más que las ganancias del mismo tamaño (Kahneman y Tversky, 1979, teoría de
  las perspectivas).
- Los inversores tienden a **vender rápido lo que gana y aguantar lo que pierde** (el "efecto
  disposición": Shefrin y Statman, 1985). Odean (1998) lo midió en las cuentas de 10.000 clientes
  de un broker: preferían realizar las ganancias antes que las pérdidas.

Con el plan escrito, mover el stop "para darle aire" o cerrar antes "por las dudas" queda a la
vista en tu propia nota.

> **Probá antes de seguir:** tu plan decía stop en 5.980. El precio llega a 5.985 y movés el stop a
> 5.960 "para darle aire". Termina tocando 5.960. ¿Qué anotás?
>
> <details><summary>Ver la respuesta</summary>
>
> Que rompiste el plan: moviste el stop en contra y la pérdida fue más grande que el riesgo
> planeado. Es exactamente el efecto disposición: aguantar lo que pierde.
>
> </details>

## Interactivo

En el replay, el campo **Plan** aparece antes de confirmar una entrada; al terminar la sesión, el
resultado muestra primero si cumpliste cada regla del proceso y recién después el resultado en
[R](../GLOSARIO.md#r-multiplo).

## Ejercicio

En tu próximo replay de práctica, escribí el plan de una entrada con los tres puntos de arriba y,
al final, una nota de tres líneas: ¿seguí el plan?, ¿qué pasó?, ¿qué cambio?

<details><summary>Ver la solución</summary>

Una nota útil se puede revisar dentro de un mes: dice qué regla seguiste o rompiste, no solo
"gané" o "perdí". Ejemplo: "Seguí el plan. Stop en 5.980 tocado a las 4 velas. La idea (rebote en
6.000) no se dio; el nivel ya tenía tres toques y lo rompió el cuarto."

</details>

## Examen

Esta lección no tiene examen: es un hábito. Se completa al leerla y se practica en cada replay.

## Desbloquea

- **Notas del Diario**: escribir una nota después de cada ejercicio.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Operaciones mínimas de un examen de operaciones (provisorio) | 10 (`10`) | `config/academy.toml` `replay.trades_per_exam` | A |
| Regla del proceso: plan escrito antes de la entrada | `plan_written` | `engine/crates/academy/src/grade.rs` | A |
| El plan de una entrada tiene hasta 2.000 caracteres | `plan de hasta 2000` | `engine/crates/academy/src/replay.rs` | A |
| Un hábito por nivel, desde el nivel 3 | `un hábito por nivel` | `docs/design/2026-11-academia.md` §2 | A |
| Las pérdidas pesan más que las ganancias | — | [Kahneman y Tversky (1979), *Prospect Theory: An Analysis of Decision under Risk*, Econometrica 47(2)](https://www.econometricsociety.org/publications/econometrica/1979/03/01/prospect-theory-analysis-decision-under-risk), leído el 26-11-2026 | A |
| Efecto disposición | — | [Shefrin y Statman (1985), *The Disposition to Sell Winners Too Early and Ride Losers Too Long*, Journal of Finance 40(3)](https://doi.org/10.1111/j.1540-6261.1985.tb05002.x) | A |
| 10.000 cuentas de un broker: prefieren realizar ganancias | — | [Odean (1998), *Are Investors Reluctant to Realize Their Losses?*, Journal of Finance 53(5)](https://faculty.haas.berkeley.edu/odean/papers/disposition/disposition.html), resumen leído el 26-11-2026 | A |
