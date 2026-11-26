# 8.2 Rachas

> **Nivel 8 · Expectativa, varianza y suerte.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Las
> probabilidades de esta lección son cuentas exactas con operaciones independientes: no son
> resultados de ninguna estrategia.

## Objetivo

Saber qué racha de pérdidas es **normal** antes de que pase: con 50 % de aciertos, **10 pérdidas
seguidas en 1.000 operaciones** pasan en más de 1 de cada 3 series. Una racha así, sola, no prueba
que la regla se rompió.

## La idea

La intuición falla con las rachas. Cuando a la gente se le pide inventar 200 tiradas de una
moneda, casi nunca escribe rachas largas; las monedas de verdad sí las tienen (Schilling, 1990). Con
las operaciones pasa lo mismo.

**1. La racha más larga crece como el logaritmo de n.** Para una moneda justa (50 % de aciertos),
la racha más larga de pérdidas en n operaciones es, en promedio:

racha esperada ≈ log₂(n) − 2/3

- 100 operaciones: ≈ 6,0 pérdidas seguidas.
- 1.000 operaciones: ≈ 9,3.
- 10.000 operaciones: ≈ 12,6.

Multiplicar las operaciones por 10 suma solo unas 3,3 pérdidas a la racha. Y el desvío es casi
fijo, ≈ 1,87: la racha más larga casi siempre cae a unas 2 de su promedio.

**2. Con menos aciertos, rachas más largas.** Si cada operación pierde con probabilidad q (y gana
con 1 − q), la fórmula general de Schilling es log en base 1/q de (n × (1 − q)), más una
corrección chica. Una regla con 40 % de aciertos (q = 0,6) y 1.000 operaciones espera una racha de
≈ **12,4** pérdidas. Las reglas de objetivo lejano (2 R o 3 R) aciertan poco: sus rachas son largas
**por diseño**, no por mala suerte.

**3. La cuenta exacta.** Con la recursión de Schilling (cuántas secuencias de n operaciones no
tienen ninguna racha de k pérdidas) se calcula la probabilidad exacta de que la racha más larga sea
de **k o más**, en 1.000 operaciones:

| Racha de k o más pérdidas | 50 % de aciertos | 40 % de aciertos |
|---|---|---|
| 7 | 98,2 % | ≈ 100 % |
| 8 | 86,1 % | 99,9 % |
| 9 | 62,4 % | 98,5 % |
| 10 | **38,5 %** | 91,5 % |
| 12 | 11,4 % | 58,2 % |
| 15 | 1,5 % | 17,0 % |

Con 100 operaciones y 50 % de aciertos, en cambio, una racha de 10 o más pasa solo el 4,4 % de las
veces: la misma racha es rara en una muestra corta y común en una larga.

**4. Lo que cuesta una racha depende del tamaño.** Arriesgando un porcentaje fijo del capital del
momento, 10 pérdidas seguidas de 1 R dejan:

- al **1 %** (nuestro riesgo por operación): 0,99¹⁰ = 0,904 → **−9,6 %**;
- al **5 %**: 0,95¹⁰ = 0,599 → **−40,1 %**, y para volver hace falta ganar +67 %.

La racha es la misma; el daño lo elige el tamaño. Por eso el riesgo por operación es chico, y por
eso la reducción gradual de la lección 7.4 achica el tamaño cuando la cuenta está en caída.

**5. Supuesto importante.** Todo esto supone operaciones **independientes**. Si las pérdidas
vienen juntas (por ejemplo, porque el mercado cambió de régimen), las rachas reales son más largas
que las de la tabla. La lección 8.3 lo muestra con una serie real.

> **Probá antes de seguir:** una regla con 50 % de aciertos lleva 300 operaciones y acaba de
> perder 8 seguidas. ¿Es motivo para abandonarla?
>
> <details><summary>Ver la respuesta</summary>
>
> No por sí solo. La racha esperada en 300 operaciones es ≈ log₂(300) − 2/3 ≈ 7,6: 8 está dentro de
> lo normal. Lo que sí hay que revisar es si la regla se sigue aplicando bien y si el mercado cambió;
> la decisión se toma con la muestra entera, no con la racha.
>
> </details>

## Interactivo

El simulador de varianza del Laboratorio (se abre en la lección 8.3): win rate 50 %, ganadora 1 R,
perdedora 1 R y 1.000 operaciones. El motor sortea 10.000 caminos con semilla fija y da la racha
más larga de cada camino (p5, p50 y p95) y la probabilidad de una racha de k o más, de 1 a 20. Los
números tienen que dar cerca de la tabla de arriba (la diferencia es el error de simular).

## Ejercicio

Antes de mirar la tabla, anotá cuál creés que es la racha de pérdidas más larga en 1.000
operaciones con 50 % de aciertos. Después compará con 9,3 y con la tabla. ¿Te quedaste corto?

<details><summary>Ver la solución</summary>

Casi todos se quedan cortos (dicen 4 o 5). La racha más larga esperada es ≈ 9, y 10 o más pasa en
el 38,5 % de las series. Si tu plan de trading no aguanta 10 pérdidas seguidas sin cambiar las
reglas, el plan no está listo.

</details>

## Examen

6 preguntas (cálculos y conceptos). Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo: el simulador de varianza se abre en la lección 8.3.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Recursión exacta de la racha más larga; racha esperada ≈ log₂(n) − 2/3 con 50 %; desvío ≈ 1,873; fórmula general con otra probabilidad; la gente inventa secuencias sin rachas largas | — | [Schilling (1990), *The Longest Run of Heads*, The College Mathematics Journal 21(3), 196–207](https://www.jstor.org/stable/2686886), texto completo leído el 26-11-2026; la tabla sale de su recursión | A |
| Riesgo por operación | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
| El simulador de varianza da la probabilidad de cada racha | `P(racha perdedora más larga ≥ k)` | `engine/crates/backtest/src/research/variance.rs` | A |
| Rachas de 1 a k en el simulador | k hasta 20 (`20`) | `config/lab.toml` `variance.streak_max_k` | A |
| Caminos del simulador | `10000` | `config/lab.toml` `variance.default_paths` | A |
