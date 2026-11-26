# 10.4 Regímenes

> **Nivel 10 · Tu propia estrategia.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del
> pasado: no dicen nada seguro del futuro.

## Objetivo

Ver cómo **el mismo sistema cambia con el estado del mercado** (volatilidad y tendencia) y por qué
cortar los resultados por régimen es fácil de hacer mal.

## La idea

Un **régimen** es el estado del mercado en un período: volatilidad alta o baja, tendencia o rango,
calma o crisis. Una regla puede ganar en uno y perder en otro, y el promedio de todo el período
esconde esa diferencia.

**1. Un caso nuestro: el tercio bajo de volatilidad.** En el diagnóstico de la estrategia en uso
(15 minutos), las operaciones del **tercio bajo del ATR de 15 m** son las peores en los tres
índices: −0,35, −0,24 y −0,22 R por operación. Y pagan de **3 a 6 veces más costos** que las del
tercio alto. Con poca volatilidad los stops son cortos, y el mismo spread pesa mucho más en R
(nivel 8.1). El régimen no cambia la señal: cambia cuánto se come el costo.

**2. La trampa del corte con el diario de hoy.** Ese tercio se calculó con cortes de **toda la
muestra**: para saber si una operación de 2014 era de «volatilidad baja» se usaron datos hasta
2025. Sirve para **describir**, no para **decidir**: en vivo esos cortes no se conocen (es mirar el
futuro, nivel 9.1). El tercio oficial, que solo mira los 3 años anteriores a cada día, **no da un
patrón común** en los tres índices.

**3. Efectos que dependen de una época.**

- La deriva nocturna de 02:00 a 03:00 ET era real en 2013–2020 (t 4,40 en US500) y **desaparece
  desde 2021** (R022). Encima, el costo del CFD era de 2 a 4 veces mayor (R021).
- H2 (días de CPI y empleo) tuvo una ventaja media de +0,044 % por mes en 2013–2020 y −0,174 % en
  2021–2025 (R031).

Por eso cada pre-registro exige que la ventaja exista **en las dos mitades**: una regla que vive en
un solo régimen es frágil.

**4. Usar la volatilidad sin mirar el futuro.** Hay formas honestas, con datos que ya existían:

- el **régimen del VIX** que muestra la app (nivel 6): calma, normal desde 15, elevado desde 20 y
  estrés desde 30. Es contexto: no entra al puntaje;
- un tamaño que baja cuando la volatilidad sube (exposición manejada por volatilidad, Moreira y
  Muir, 2017). Nuestra prueba V en los tres índices **no pasó**: +0,164 % por mes contra comprar y
  mantener, con p 0,207 (R019).

El régimen «volatilidad × tendencia» por instrumento del motor (`regime`) está **planeado**, no
hecho: por eso esta lección no abre ninguna capa nueva.

**5. Mirar cortes es probar más.** Cortar los resultados por año, hora, día, dirección y régimen
siempre encuentra algún grupo que gana. Cada corte es una prueba: lo que aparece ahí es una
**idea** para pre-registrar y probar después, no una regla para operar. El informe del Laboratorio
muestra los cortes con ese aviso.

> **Probá antes de seguir:** una regla gana en los meses de VIX elevado y pierde en los de calma.
> ¿La operás solo con el VIX elevado?
>
> <details><summary>Ver la respuesta</summary>
>
> Todavía no. Primero: ¿el corte se hizo con el VIX conocido **antes** de cada operación? Segundo:
> ¿cuántos cortes miraste hasta encontrar ese? Si las dos respuestas están bien, es una hipótesis
> nueva: se escribe completa, pasa el filtro de borde bruto y se pre-registra (lecciones 10.1 y
> 10.2).
>
> </details>

## Interactivo

Resultados cortados por régimen (los del diagnóstico de la estrategia en uso y los cortes del
informe del Laboratorio), cada uno con su aviso de que es descriptivo. El régimen del VIX ya está
abierto desde la lección 6.4.

## Ejercicio

¿Cuál de estos filtros de régimen se puede usar en un backtest sin mirar el futuro?

1. «Volatilidad alta» = ATR de hoy en el tercio alto de todos los ATR de 2011 a 2025.
2. «Volatilidad alta» = ATR de ayer arriba del percentil 67 de los 3 años anteriores.
3. «Mercado en tendencia» = el año terminó arriba de donde empezó.

<details><summary>Ver la solución</summary>

Solo el 2: usa datos que ya existían. El 1 usa cortes de toda la muestra (futuro) y el 3 usa el
cierre del año, que se conoce recién al final.

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo. La «capa de régimen» del temario no existe todavía en el motor (el régimen volatilidad
× tendencia está planeado); el régimen del VIX se abrió en la lección 6.4.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Tercio bajo del ATR, el peor en los tres | `es el peor en los tres: −0,35, −0,24 y −0,22 R` | `docs/analysis/2026-11-diagnostico-us-futuros.md` | A |
| Paga de 3 a 6 veces más costos | `El tercio bajo del ATR paga de 3 a 6 veces más costos que el alto` | `docs/analysis/2026-11-diagnostico-us-futuros.md` | A |
| El tercio oficial no da un patrón común | `El tercio oficial de volatilidad no da un patrón común` | `docs/analysis/2026-11-diagnostico-us-futuros.md` | A |
| El sustituto descriptivo no sirve para decidir | `no sirve para decidir en vivo` | `docs/analysis/2026-11-diagnostico-de40-futuros.md` | A |
| Deriva nocturna por mitades | `US500 +1.53 / -0.31 pb (t 4.40 / -0.79)` | `docs/analysis/registro-de-resultados.csv` `R022` | A |
| Deriva nocturna: el costo del CFD es mayor | `real pero el costo del CFD es 2 a 4 veces mayor` | `docs/analysis/registro-de-resultados.csv` `R021` | A |
| H2 por mitades (USD) | `+0.044 % / -0.174 %` | `docs/analysis/registro-de-resultados.csv` `R031` | A |
| Exposición manejada por volatilidad (USD) | `+0.164 % (p 0.207` | `docs/analysis/registro-de-resultados.csv` `R019` | A |
| VIX: normal desde | 15 (`15`) | `config/analysis.toml` `vix.normal_from` | A |
| VIX: elevado desde | 20 (`20`) | `config/analysis.toml` `vix.elevated_from` | A |
| VIX: estrés desde | 30 (`30`) | `config/analysis.toml` `vix.stress_from` | A |
| El régimen del motor está planeado | `régimen planeado` | `docs/modules/regime.md` | A |
| Los cortes del informe llevan aviso | `mirar cortes es probar más` | `docs/design/2026-11-laboratorio.md` | A |
| Moreira y Muir (2017) | `Moreira y Muir (2017)` | `docs/design/2026-11-academia.md` | A |
