# 8.4 Suerte y habilidad

> **Nivel 8 · Expectativa, varianza y suerte.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los números
> de los ejemplos son redondos, de práctica: no son resultados de ninguna estrategia.

## Objetivo

Separar una **buena decisión** de un **buen resultado**, y calcular cuántas operaciones hacen falta
para distinguir una ventaja del ruido.

## La idea

**1. Un buen resultado no prueba una buena decisión.** Annie Duke, jugadora profesional de póker,
llama *resulting* a atar demasiado la calidad de una decisión a la calidad de su resultado. Una
operación sin stop que ganó fue una mala decisión con buena suerte; una operación con el plan, el
stop y el tamaño correctos que perdió fue una buena decisión con mala suerte. Por eso la Academia
califica el **proceso** y muestra el R aparte (nivel 7).

**2. Cuánto se mueve un promedio por suerte.** Si cada operación tiene un desvío σ (en R), el
promedio de N operaciones se mueve alrededor de la expectativa con un **error estándar**:

error estándar = σ ÷ √N

y el estadístico **t** = expectativa ÷ error estándar dice cuántos "errores" separan el promedio
de cero. Es lo que calcula el motor en sus pruebas t.

**El mismo jugador, con 100 y con 1.000 operaciones.** Una regla con expectativa real de +0,1 R y
σ = 1 R:

| Operaciones | Error estándar | t esperado | El promedio medido cae, el 95 % de las veces, entre | Probabilidad de ver un promedio negativo |
|---|---|---|---|---|
| 100 | 0,10 R | 1 | −0,10 y +0,30 R | ≈ 16 % |
| 400 | 0,05 R | 2 | 0,00 y +0,20 R | ≈ 2 % |
| 1.000 | 0,032 R | 3,2 | +0,04 y +0,16 R | ≈ 0,1 % |

Y al revés: un jugador **sin** ventaja (E = 0) muestra un promedio de +0,1 R o más en 100
operaciones el 16 % de las veces. De seis jugadores sin ventaja, uno "parece bueno" después de 100
operaciones.

**3. Cuántas operaciones hacen falta.** Para llegar a un t dado:

N = (t × σ ÷ E)²

- E = 0,1 R y σ = 1 R: t = 2 pide **400** operaciones; t = 3, **900**.
- Una regla de 2 R de objetivo y 36 % de aciertos: E = 0,08 R y σ = 3 × √(0,36 × 0,64) = 1,44 R.
  Para t = 2 hacen falta (2 × 1,44 ÷ 0,08)² = **1.296** operaciones. Las reglas de objetivo lejano
  necesitan muestras más grandes.

**4. Por qué la vara es más alta que t = 2.** Cuando se prueban muchas ideas, alguna da t = 2 por
suerte. Harvey, Liu y Zhu (2016) piden **t > 3** para un factor nuevo en bolsa, porque ya se
probaron cientos. Nuestra vara: p < 0,0125 unilateral por hipótesis (t ≈ 2,24 con muestras
grandes) **y** un Sharpe deflactado de al menos 0,95, que descuenta todas las pruebas del proyecto
(nivel 9).

**5. El test de suerte.** Otra manera de separar suerte de habilidad: comparar la estrategia con
**muchas entradas al azar con las mismas salidas y el mismo tamaño** (1.000 en el Laboratorio). Si
la estrategia no le gana a la mayoría, la entrada no aporta nada. Ejemplos nuestros:

- H4.1 (Bruni literal): percentil 93 a 97 del test de suerte, pero casi todo de 2016–2020; desde
  2021 (428 operaciones) queda en +0,007 R por operación.
- R040 (cruces de EMA, exploratorio): percentil 77 a 94. Mejor que la mayoría de las entradas al
  azar, pero lejos de ser raro.

Un percentil alto es una pista, no una prueba: con muchas variantes, alguna queda arriba por suerte.

> **Probá antes de seguir:** un trader muestra 30 operaciones con +0,5 R de promedio y σ = 2 R.
> ¿Cuál es su t?
>
> <details><summary>Ver la respuesta</summary>
>
> Error estándar = 2 ÷ √30 ≈ 0,365 R; t ≈ 0,5 ÷ 0,365 ≈ **1,37**. No se distingue del azar, aunque
> el promedio "se vea" muy bueno.
>
> </details>

## Interactivo

El mismo jugador con 100 y con 1.000 operaciones: el simulador de varianza (lección 8.3) con
expectativa positiva, mirando cuántos caminos terminan abajo después de 100 operaciones y cuántos
después de 1.000.

## Ejercicio

Sin nota: decidí, para cada caso, si el resultado alcanza para hablar de ventaja.

1. 12 operaciones, 10 ganadoras.
2. 1.500 operaciones, +0,08 R de promedio, σ = 1,2 R.
3. 200 operaciones, +0,2 R de promedio, σ = 1 R, elegida entre 50 variantes probadas.

<details><summary>Ver la solución</summary>

1. No: 12 operaciones no dicen casi nada, aunque 10 de 12 parezca mucho.
2. Error estándar = 1,2 ÷ √1.500 ≈ 0,031 R; t ≈ 2,6. Prometedor, pero hay que saber cuántas cosas se
   probaron antes (nivel 9) y si aguanta costos más altos.
3. t = 0,2 ÷ (1 ÷ √200) ≈ 2,8, pero es la mejor de 50: con tantas pruebas, un t así aparece por
   suerte. Hace falta corregir por la búsqueda (Sharpe deflactado, nivel 9) y probar fuera de
   muestra.

</details>

## Examen

6 preguntas (cálculos y conceptos). Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| *Resulting*: atar demasiado la calidad de la decisión a la del resultado | — | Annie Duke (2018), *Thinking in Bets*, Portfolio/Penguin; definición en sus palabras en [la entrevista de Nautilus (2017)](https://nautil.us/the-resulting-fallacy-is-ruining-your-decisions-236901), leída el 26-11-2026 | B |
| El estadístico t del motor | `let t = m / (s / (n as f64).sqrt());` | `engine/crates/backtest/src/research/stats.rs` | A |
| t > 3 para un factor nuevo | — | [Harvey, Liu y Zhu (2016), *…and the Cross-Section of Expected Returns*, Review of Financial Studies 29(1)](https://www.nber.org/papers/w20592), resumen leído el 26-11-2026 | A |
| Nivel de cada hipótesis | 1,25 % (`0.0125`) | `config/research.toml` `rule.alpha` | A |
| Sharpe deflactado mínimo | `0.95` | `config/research.toml` `rule.dsr_min` | A |
| Entradas al azar del test de suerte | `1000` | `config/lab.toml` `report.luck_simulations` | A |
| H4.1: percentil del test de suerte | `percentil 93 a 97 del test de suerte` | `docs/analysis/2026-11-programa-4-h41-h42-resultados.md` | A |
| H4.1 desde 2021 | `desde 2021 (428 operaciones) queda en` | `docs/analysis/2026-11-programa-4-h41-h42-resultados.md` | A |
| R040: percentil del test de suerte | `suerte en el percentil 77 a 94` | `docs/analysis/registro-de-resultados.csv` `R040` | A |
