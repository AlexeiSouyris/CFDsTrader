# 4.4 Evidencia: soportes y números redondos

> **Nivel 4 · Estructura.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del pasado, con
> datos de futuros: no dicen nada seguro del futuro.

## Objetivo

Saber qué dicen los estudios serios sobre soportes, resistencias y
[números redondos](../GLOSARIO.md#numero-redondo), y qué dio **nuestra** prueba con los índices.

## La idea

**Lo que encontró Osler en divisas.**

- **Osler (2000):** tomó los soportes y resistencias que **seis firmas** publicaban para sus
  clientes en el mercado de divisas. Encontró evidencia fuerte de que los niveles **ayudaban a predecir**
  dónde se interrumpía una tendencia dentro del día; pero ese poder **variaba** según la firma y la
  moneda.
- **Osler (2003):** con datos de órdenes reales de **stop** y de **tomar ganancia** en divisas
  (los primeros que se publicaron), vio que las de tomar ganancia se juntan en los números redondos
  (eso puede frenar el precio ahí) y las de stop se juntan **justo después** de los números
  redondos (eso puede acelerar el precio al cruzarlos).

**Lo que encontró Lo, Mamaysky y Wang (2000).** Definieron los patrones con un algoritmo (sin
mirar "a ojo") en acciones de EE. UU. de 1962 a 1996: algunos patrones agregaban **algo de
información** sobre los retornos. Es un resultado de información, no de ganancias después de
costos.

**Lo que dio nuestra prueba (R020).** Probamos la idea de Osler más "operable": seguir el precio
cuando **cruza** un número redondo (múltiplos de 50 en US500, de 100 en US100 y de 500 en US30), con
stop a 2 ATR y salida a las 2 horas. Para saber si el número redondo importa, la comparamos con un
**control**: la misma regla en los niveles intermedios (5.025, 5.075...).

| | Resultado |
|---|---|
| Cartera, 153 meses (02-2013 a 10-2025) | **−0,62 % por mes** (t −2,37) |
| Antes de costos, cruzando números redondos | +0,041 R por operación |
| Antes de costos, cruzando niveles intermedios (control) | **+0,047 R** por operación |

El número redondo **no agregó nada**: el control dio lo mismo o un poco más. Después de costos,
pierde. **No pasó** y la idea quedó cerrada para estos datos.

**Cómo se leen juntas.** Osler midió divisas, con datos de hace más de 20 años. Nosotros
medimos futuros de índices de 2013 a 2025. Que un efecto exista en un mercado y una época no
garantiza que exista en otro. Además, nuestros números redondos son los del **futuro**, que no
coinciden con los del CFD (la diferencia por la base es de decenas de puntos).

> **Probá antes de seguir:** ¿para qué sirve el control de los niveles intermedios?
>
> <details><summary>Ver la respuesta</summary>
>
> Para separar el efecto del número redondo del efecto de cualquier cruce. Si cruzar 5.025 da lo
> mismo que cruzar 5.050, lo que se mide es solo "el precio siguió un poco después de moverse", no
> algo especial del número redondo.
>
> </details>

## Interactivo

La fila R020 del registro de resultados con su informe, y los números redondos del mapa de
liquidez sobre un gráfico real (nivel 5).

## Ejercicio

Escribí en una frase por qué un estudio en divisas de hace más de 20 años no alcanza para operar números
redondos en el US500 de hoy.

<details><summary>Ver la solución</summary>

Porque es otro mercado y otra época, y el efecto puede no estar (o ser más chico que los costos)
en este instrumento: por eso lo probamos con nuestros datos, con un control y con costos, y no
pasó.

</details>

## Examen

Esta lección no tiene examen: es la evidencia del nivel. Se completa al leerla.

## Desbloquea

Nada nuevo: las capas de estructura se abren en 4.1 a 4.3.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| M1: −0,62 % por mes, t −2,37 | `-0.62 % (t -2.37` | `docs/analysis/registro-de-resultados.csv` `R020` | A |
| Antes de costos: redondo / control | `+0.041 / +0.047` | `docs/analysis/registro-de-resultados.csv` `R020` | A |
| Período de la prueba | `2013-02 a 2025-10 (153 meses)` | `docs/analysis/registro-de-resultados.csv` `R020` | A |
| Paso de los números redondos en US500 | 50 (`50`) | `config/research.toml` `round_number.steps.US500` | A |
| Paso en US100 | 100 (`100`) | `config/research.toml` `round_number.steps.US100` | A |
| Paso en US30 | 500 (`500`) | `config/research.toml` `round_number.steps.US30` | A |
| Salida a las 2 horas | 120 minutos (`120`) | `config/research.toml` `round_number.horizon_minutes` | A |
| Stop a 2 ATR | 2 (`2`) | `config/research.toml` `round_number.stop_atr` | A |
| Control: la misma regla en los niveles intermedios | `Control: la misma regla con los niveles intermedios` | `docs/analysis/2026-11-ronda3-volatilidad-y-numeros-redondos.md` | A |
| Los redondos medidos son los del futuro, no los del CFD | `Futuros, no el CFD` | `docs/analysis/2026-11-ronda3-volatilidad-y-numeros-redondos.md` | A |
| Seis firmas; los niveles predicen interrupciones; varía por firma y moneda | — | [Osler (2000), *Support for Resistance: Technical Analysis and Intraday Exchange Rates*, FRBNY Economic Policy Review 6(2)](https://www.newyorkfed.org/research/epr/00v06n2/0007osle.html), resumen leído el 26-11-2026 | A |
| Tomar ganancia en los redondos; stops justo después | — | [Osler (2003), *Currency Orders and Exchange Rate Dynamics*, Journal of Finance 58(5)](https://ideas.repec.org/a/bla/jfinan/v58y2003i5p1791-1819.html), resumen leído el 26-11-2026 | A |
| Patrones definidos con un algoritmo: algo de información | — | [Lo, Mamaysky y Wang (2000), *Foundations of Technical Analysis*, Journal of Finance 55(4) (versión NBER w7613)](https://www.nber.org/papers/w7613), resumen leído el 26-11-2026 | A |
