# 0.3 Cómo se usa la Academia

> **Nivel 0 · Orientación.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Saber las tres reglas de la Academia antes de usarla: **simulación siempre**, **proceso antes que
resultado** y **nada de promesas**.

## La idea

**1. Simulación siempre.** La app no manda órdenes reales. Todo lo que hagas acá (ejercicios,
exámenes, el ticket) es [paper trading](../GLOSARIO.md#paper-trading): precios reales, dinero
ficticio. Pasar a dinero real no es una opción de la app: necesita una decisión escrita (un ADR)
aprobada por dos personas, y hoy está bloqueado.

**2. Proceso antes que resultado.** Una operación que ganó sin [stop](../GLOSARIO.md#stop-loss)
**reprueba**. Una que perdió pero siguió el plan (stop, tamaño, costos) **aprueba**. El motor
califica si seguiste las reglas, no si tuviste suerte. El resultado en
[R](../GLOSARIO.md#r-multiplo) se muestra aparte, para que lo leas, pero no decide la nota.

**3. Nada de promesas.** Ninguna lección te promete ganancias. Cuando algo **no funcionó** en
nuestras pruebas, la lección lo dice con su número del registro de resultados. Vas a ver varios
casos así desde el nivel 2.

**Cómo está armada.** Cada lección tiene siempre las mismas partes:

| Parte | Qué es |
|---|---|
| Objetivo | Qué vas a saber hacer al terminar. |
| Interactivo | Algo para tocar, con datos que manda el motor. |
| Ejercicio | Práctica sin nota, con la solución al lado. |
| Examen | Con nota. Se puede repetir; cada intento queda guardado. |
| Desbloquea | La herramienta de la Terminal que se abre al aprobar. |

Al final de cada lección hay una tabla **Cifras y fuentes**: cada número dice de dónde sale y qué
tan fuerte es su prueba (**A** = dato oficial o artículo revisado, **B** = libro con método
explícito, **C** = verificado solo en parte). Los números que salen de nuestra config los compara
el motor con la config real: si la config cambia, la lección no pasa sus pruebas hasta que se
corrija.

**Los candados.** Las herramientas bloqueadas se ven en gris con un candado y te dicen qué lección
las abre. El dueño las puede desbloquear a mano en Ajustes; queda registrado. Desbloquear no cambia
nada del motor: el riesgo, los límites y el kill switch siguen iguales.

**El pasado que no se toca.** Los exámenes con velas del pasado nunca usan los últimos 12 meses de
datos: ese tramo está reservado para juzgar estrategias al final, y si lo "aprendés" en un examen,
después no sirve para juzgar nada.

> **Probá antes de seguir:** hacés una operación sin stop y gana 3 R. ¿Aprueba?
>
> <details><summary>Ver la respuesta</summary>
>
> No. Sin stop rompiste la regla más importante, aunque esta vez saliera bien. La nota es de
> proceso.
>
> </details>

## Interactivo

Un recorrido por la Terminal con todo bloqueado: cada candado dice qué lección lo abre y por qué
conviene aprender eso antes de usar la herramienta.

## Ejercicio

Recorré la Terminal y anotá tres herramientas bloqueadas y la lección que abre cada una.

## Examen

Esta lección no tiene examen: se completa al leerla.

## Desbloquea

Nada: es la puerta de entrada.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Meses fuera de muestra que no se tocan | 12 (`12`) | `config/data_split.toml` `out_of_sample.months` | A |
| Umbral para aprobar un examen de preguntas (provisorio) | 80 % (`80`) | `config/academy.toml` `thresholds.quiz_pass_pct` | A |
| Herramientas bloqueadas visibles con candado | sí (`true`) | `config/academy.toml` `thresholds.locked_tools_visible` | A |
| Los umbrales son provisorios hasta que el dueño los apruebe | sí (`true`) | `config/academy.toml` `thresholds.provisional` | A |
| Modo por defecto: simulación; el dinero real pide un ADR aprobado por dos personas | `simulación/paper` | `CLAUDE.md` §5 (reglas del proyecto) | A |
| Una operación ganadora sin stop reprueba (proceso antes que resultado) | `Una operación ganadora sin stop` | `docs/design/2026-11-academia.md` §1 (principios) | A |
