# 8.3 Monte Carlo

> **Nivel 8 · Expectativa, varianza y suerte.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son
> resultados del pasado, remuestreados: no dicen nada seguro del futuro.

## Objetivo

Leer **muchos caminos posibles** de la misma serie de resultados: qué [caída](../GLOSARIO.md#drawdown)
máxima, qué racha perdedora, cuánto tiempo bajo el agua y qué riesgo de ruina son **normales**, y
cuándo algo **no** es normal.

## La idea

Un backtest da **un** camino: el orden en que pasaron las cosas. Pero el mismo resultado medio pudo
venir en otro orden, con las pérdidas juntas al principio. El Monte Carlo arma miles de órdenes
posibles con los mismos datos y mira cómo varían.

**1. Cómo lo hace el motor** (`research::montecarlo`, valores de `config/research.toml`):

- Toma la serie real (por ejemplo, 153 retornos mensuales) y arma un camino nuevo pegando **bloques
  de 5 meses seguidos** que empiezan en lugares sorteados, dando la vuelta al final de la serie
  (bootstrap circular por bloques, Politis y Romano, 1992). Los bloques conservan los meses malos
  que vienen juntos; con meses sueltos, las rachas salen demasiado cortas.
- Repite **10.000 caminos** con una semilla fija: misma serie y misma semilla, mismo resultado.
- En cada camino mide la caída máxima desde el pico, la racha perdedora más larga, el tramo más
  largo bajo el máximo anterior, el retorno final y si hubo **ruina**: tocar **−30 % del capital
  inicial** en algún momento.
- Resume cada medida con **p5, p50 y p95**: el p95 de la caída es la caída que el 95 % de los
  caminos no superó; 1 de cada 20 caminos fue peor.

**2. Una serie real: comprar y mantener** (US500, US100 y US30 con los costos del CFD, R018, 153
meses, escenario USD; R041):

| | Real | p5 | p50 | p95 |
|---|---|---|---|---|
| Caída máxima | 29,55 % | 14,02 % | 21,93 % | 35,58 % |
| Meses bajo el agua (tramo más largo) | 30 | — | 25 | 63 |

Riesgo de ruina: **1,26 %** de los caminos. Cómo se lee: la caída real (29,55 %) queda entre el
p50 y el p95, o sea, **dentro de lo normal** para esa serie. Y el camino mediano ya cae 22 %: quien
mantiene estos índices tiene que esperar caídas de 20 % a 35 % como parte del viaje, no como
"algo que se rompió".

**3. Cuando el real queda afuera: H2 en US100** (días de CPI y empleo, R031; R041): la racha
perdedora real fue de **11 meses**, y el p95 de los caminos es **10**. La serie real tuvo una racha
más larga que el 95 % de sus propias reordenaciones. Lo más probable no es "mala suerte rara", sino
algo que el remuestreo por bloques **no reproduce**: H2 cambió entre las dos mitades (su ventaja
media frente a comprar y mantener pasó de +0,044 % por mes en 2013–2020 a −0,174 % en 2021–2025).
El Monte Carlo no explica el porqué; avisa dónde mirar.

**4. Lo que no hace.** El Monte Carlo **reordena el pasado**: supone que el futuro se parece a ese
pasado y no agrega información nueva. Una estrategia sin ventaja sigue sin ventaja en los 10.000
caminos: en R039, la estrategia en uso con y sin topes tiene una caída p95 de 97,4 % y 99,6 %, y la
ruina aparece en casi todos los caminos. Y en ese informe el Monte Carlo remuestrea los retornos
mensuales ya realizados: no vuelve a aplicar las reglas de riesgo en cada camino.

**5. El simulador de varianza del Laboratorio** (lo abre esta lección) hace caminos de
**operaciones** en vez de meses:

- Desde un **modelo** (win rate, R medio ganador y perdedor, riesgo por operación) o desde los R
  reales de un informe del Laboratorio, remuestreados.
- Hasta 10.000 caminos de hasta 1.000 operaciones; ruina por defecto en −30 %.
- Deja simular hasta un 10 % de riesgo por operación, más que el del proyecto **a propósito**: para
  ver la ruina. El informe avisa cuando pasás el riesgo del proyecto.
- Opción de **incertidumbre del win rate**: un win rate medido con pocas operaciones no es cierto;
  el 55 % "medido" puede ser 48 %. Cada camino sortea su win rate y la ruina sube.

> **Probá antes de seguir:** en el simulador ponés 50 % de aciertos, ganadoras de 1,2 R, perdedoras
> de 1 R y 2 % de riesgo. ¿La ruina va a ser cero porque la expectativa es positiva?
>
> <details><summary>Ver la respuesta</summary>
>
> No. La expectativa es +0,1 R por operación, pero con 2 % por operación una racha de 15 o 20
> pérdidas (lección 8.2) deja la cuenta en −26 % a −33 %: algunos caminos tocan la ruina antes de
> que la ventaja aparezca. Expectativa positiva no es lo mismo que riesgo cero.
>
> </details>

## Interactivo

El abanico de caminos de una serie real del registro (comprar y mantener, R041) con sus bandas p5,
p50 y p95, y el mismo abanico en el simulador de varianza con tus parámetros.

## Ejercicio

En la tabla de comprar y mantener: (1) ¿cuál es la caída del camino mediano? (2) ¿Qué parte de los
caminos tuvo una caída peor que 35,58 %? (3) La caída real fue de 29,55 %: ¿fue normal?

<details><summary>Ver la solución</summary>

1. **21,93 %** (el p50).
2. **5 %** (1 de cada 20): eso es lo que dice el p95.
3. Sí: queda entre el p50 y el p95. Peor que la mitad de los caminos, pero lejos de lo raro.

</details>

## Examen

6 preguntas (lectura de percentiles, cálculos y conceptos). Para aprobar hace falta el 80 %
(umbral provisorio).

## Desbloquea

- **Simulador de varianza y Monte Carlo** del Laboratorio (el «visor de Monte Carlo» del temario):
  bandas de caminos, caída máxima, rachas y riesgo de ruina, desde un modelo o desde los R de un
  informe.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Caminos del Monte Carlo | `10000` | `config/research.toml` `montecarlo.paths` | A |
| Largo de cada bloque | 5 períodos (`5`) | `config/research.toml` `montecarlo.block_len` | A |
| Semilla fija | `20261125` | `config/research.toml` `montecarlo.seed` | A |
| Ruina = −30 % del capital inicial | 30 % (`0.30`) | `config/research.toml` `montecarlo.ruin_threshold` | A |
| Bootstrap circular por bloques | `Politis y Romano, 1992` | `config/research.toml` | B |
| Comprar y mantener: caída real (p5 / p50 / p95) y ruina, USD | `caída 29.55 % (14.02 / 21.93 / 35.58); ruina 1.26 %` | `docs/analysis/registro-de-resultados.csv` `R041` | A |
| Comprar y mantener: meses bajo el agua | `bajo el agua real de comprar y mantener 30 meses en USD (p50 25 / p95 63)` | `docs/analysis/registro-de-resultados.csv` `R041` | A |
| H2 en US100: racha real y p95 | `racha 11 meses (p95 10)` | `docs/analysis/registro-de-resultados.csv` `R041` | A |
| H2 en US100: mitades, USD | `+0.044 % / -0.174 %` | `docs/analysis/registro-de-resultados.csv` `R031` | A |
| Estrategia en uso con / sin topes: caída p95 | `MC p95 97.4 % / 99.6 %` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| R039 remuestrea los retornos ya realizados | `El Monte Carlo remuestrea los retornos mensuales ya realizados` | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| Simulador: caminos máximos | `10000` | `config/lab.toml` `variance.max_paths` | A |
| Simulador: operaciones máximas por camino | `1000` | `config/lab.toml` `variance.max_trades` | A |
| Simulador: ruina por defecto | 30 % (`30`) | `config/lab.toml` `variance.default_ruin_threshold_pct` | A |
| Simulador: riesgo máximo que se puede simular | 10 % (`10`) | `config/lab.toml` `variance.max_risk_pct` | A |
| Incertidumbre del win rate | `el 55 % "medido" puede ser 48 %` | `docs/design/2026-11-laboratorio.md` | A |
