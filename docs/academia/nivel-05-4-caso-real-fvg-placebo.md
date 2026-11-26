# 5.4 Caso real: el FVG frente al placebo

> **Nivel 5 · Liquidez.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa en
> simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del pasado, con datos
> de futuros: no dicen nada seguro del futuro.

## Objetivo

Leer una prueba propia completa sobre el [FVG](../GLOSARIO.md#fvg), entender qué es un **placebo**
y por qué hace falta para saber si algo "funciona".

## La idea

**La pregunta (H5.1).** Cuando el precio toca por **primera vez** un FVG de 15 m, ¿rebota más en
las 8 velas siguientes que cuando toca **un precio cualquiera** en las mismas condiciones?

**El placebo.** Para cada toque real se sortearon **5 precios "placebo"** del mismo instrumento,
el mismo mes y el mismo horario, a los que el precio llegó recorriendo la misma distancia, con el
mismo ancho de zona y después de la misma espera. Lo único que les falta es **ser un FVG**. Si el
FVG no rebota más que el placebo, el FVG no agrega nada: el rebote, si hay, es el de cualquier
precio.

**Antes de mirar.** La regla, los criterios, el nivel de significación (p < 0,025) y el período se
**congelaron antes de ver los resultados**, con un hash del documento (pre-registro). Así no se
puede "ajustar" la pregunta después.

**Lo que dio** (US100 y US500, 02-2013 a 10-2025, 153 meses):

| Prueba | Resultado | Fila |
|---|---|---|
| H5.1: real − placebo, 15 m (45.251 y 41.000 toques con placebo) | **−0,0233 ATR** por mes (t −1,80; **p 0,963**): el FVG rebota un poco **menos** que el placebo. Falla en los dos índices y en las dos mitades (−0,011 / −0,043). | R035 |
| Lo mismo en velas de 1 h (descriptivo) | **−0,063 ATR** (t −2,17): otra vez **menos** que el placebo. | R036 |
| H5.2: operaciones de la estrategia en uso que coinciden con el primer toque de un FVG a favor, frente al resto | **−0,018 R** por operación (p 0,663): **no** son mejores. US100 +0,077, US500 −0,115. | R037 |
| La estrategia filtrada: solo esas operaciones (descriptivo) | Alfa por mes contra comprar y mantener: **−0,18 %** (US100) y **−1,47 %** (US500). | R038 |

Y un "test de suerte": repitiendo 1.000 veces el sorteo de placebos, la reacción real quedó en el
**percentil 0**: casi cualquier sorteo de precios al azar rebotó más que el FVG.

**Un error, contado.** La primera corrida de H5.1 tenía un **error de código** (empezaba un mes
antes). Se corrigió, se volvió a correr y se informaron **las dos** (R034, la inválida, y R035).
Las dos dan "no pasa". Borrar la corrida mala, o quedarse con la que conviene, sería hacer trampa.

**Qué quiere decir, y qué no.** Con **estas** reglas del motor, **estos** índices y **este**
período, el primer toque de un FVG de 15 m no mostró un rebote propio, y no separó operaciones
mejores de peores. No prueba que el FVG "nunca sirva" en ningún mercado; sí dice que acá no hay
evidencia a favor, y que la frase "el precio vuelve a llenar el hueco y rebota" hay que probarla
antes de creerla.

> **Probá antes de seguir:** si el FVG rebotara en promedio +0,02 ATR, ¿alcanzaría para decir que
> funciona?
>
> <details><summary>Ver la respuesta</summary>
>
> No sin el placebo. Si un precio cualquiera en las mismas condiciones rebota +0,04 ATR, el FVG
> rebota **menos** que el azar. Eso es lo que pasó: en 15 m el FVG real dio un poco más de cero y
> el placebo más todavía.
>
> </details>

## Interactivo

El gráfico de la diferencia **real − placebo** mes por mes, con su promedio y su banda: se ve que
cambia de signo de un año a otro, sin una tendencia.

## Ejercicio

Explicá en dos frases qué es un placebo en una prueba de trading y por qué hace falta.

<details><summary>Ver la solución</summary>

Un placebo es un caso igual al real en todo (momento, horario, cómo llegó el precio, tamaño)
menos en la característica que se quiere probar. Hace falta porque el precio se mueve igual sin el
patrón: sin el placebo, el movimiento normal del mercado se confunde con un efecto del patrón.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo: las capas de FVG y sus estadísticas se abren en 5.3. Esta lección es la que dice cómo
leerlas.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| H5.1: real − placebo, 15 m | `-0.0233 ATR (t -1.80; p 0.963` | `docs/analysis/registro-de-resultados.csv` `R035` | A |
| H5.1: mitades 2013–2020 / 2021–2025 | `-0.011 / -0.043` | `docs/analysis/registro-de-resultados.csv` `R035` | A |
| H5.1: toques con placebo US100 / US500 | `45251 / 41000` | `docs/analysis/registro-de-resultados.csv` `R035` | A |
| 1 h: real − placebo | `-0.063 ATR (t -2.17` | `docs/analysis/registro-de-resultados.csv` `R036` | A |
| H5.2: F − R | `-0.018 R (p 0.663` | `docs/analysis/registro-de-resultados.csv` `R037` | A |
| H5.2: por índice US100 / US500 | `+0.077 / -0.115` | `docs/analysis/registro-de-resultados.csv` `R037` | A |
| Estrategia filtrada: alfa por mes | `-0.18 % (p 0.71) / -1.47 % (p 1.00)` | `docs/analysis/registro-de-resultados.csv` `R038` | A |
| Primera corrida inválida por un error de código | `con error de código` | `docs/analysis/registro-de-resultados.csv` `R034` | A |
| Nivel de significación de H5.1 | 0,025 (`0.025`) | `config/research.toml` `rule_program5.alpha` | A |
| Placebos por toque | 5 (`5`) | `config/research.toml` `program5.placebos_per_event` | A |
| Reacción medida a 8 velas | 8 (`8`) | `config/research.toml` `program5.horizon` | A |
| Test de suerte: la media real en el percentil 0 | `percentil 0` | `docs/analysis/2026-11-programa-5-resultados.md` §2 | A |
| Pre-registro congelado con su hash | `2026-11-programa-5-fvg-preregistro.md.sha256` | `docs/analysis/2026-11-programa-5-resultados.md` | A |
