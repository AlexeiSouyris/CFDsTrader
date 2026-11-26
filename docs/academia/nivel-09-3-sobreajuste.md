# 9.3 Sobreajuste: muchas pruebas, Sharpe deflactado y PBO

> **Nivel 9 · Backtesting y sus trampas.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Un
> [backtest](../GLOSARIO.md#backtest) es el pasado simulado: no dice nada seguro del futuro.

## Objetivo

Entender por qué **probar muchas variantes fabrica ganadoras por azar**
([sobreajuste](../GLOSARIO.md#sobreajuste)), y medirlo con el Sharpe deflactado, el PBO y el N de
pruebas del proyecto.

## La idea

**1. Cuantas más variantes, más «ganadoras» por suerte.** Si una variante sin ventaja tiene un 5 %
de chances de dar «significativa» por azar, con 20 variantes independientes la probabilidad de que
**al menos una** lo dé es 1 − 0,95²⁰ ≈ **64 %**. Si después se muestra solo esa, parece un
descubrimiento. Es lo que pasó con las reglas técnicas simples: parecían funcionar hasta que se
corrigió por todas las que se habían probado (nivel 6).

**2. El Sharpe deflactado** (Bailey y López de Prado, 2014). Calcula cuál sería el
[Sharpe](../GLOSARIO.md#sharpe) **esperado de la mejor** de N pruebas sin ninguna ventaja, y da la
probabilidad de que el Sharpe verdadero de tu estrategia esté por encima de esa vara. Tiene en
cuenta el largo de la serie, la asimetría y la curtosis. El motor lo calcula así
(`research::stats`), y para pasar pedimos **0,95**.

Cuánto sube la vara con N (153 meses, sin ventaja, Sharpe mensual del «mejor por azar», cuenta con
la fórmula del motor):

| N de pruebas | Sharpe mensual esperado del mejor por azar | Anualizado (× √12) |
|---|---|---|
| 2 | 0,04 | 0,15 |
| 10 | 0,13 | 0,44 |
| 100 | 0,21 | 0,71 |
| 361 | **0,24** | **0,83** |

Con las 361 pruebas del proyecto, una estrategia **sin ventaja** que fuera la mejor de todas
mostraría por suerte un Sharpe mensual de ≈ 0,24: lo mismo que comprar y mantener los tres índices
(0,24 con la financiación en USD, la que vale, y 0,26 en EUR; R018). Por eso nuestros Sharpe deflactados dan entre **0,000 y 0,036**: lejos de 0,95.

**3. El N del proyecto nunca se reinicia.** Arrancó en 344 (las pruebas de antes del registro) y
suma cada hipótesis que vota y cada fila de ML: hoy, **361**. El Laboratorio usa N = N del proyecto
+ las variantes distintas de la familia que probaste, **nunca menos** que el del proyecto: R040
usó N = 365 (361 + 4 variantes).

**4. El PBO** (probabilidad de sobreajuste del backtest, Bailey, Borwein, López de Prado y Zhu,
2017). Pregunta: si elijo la mejor variante en un tramo, ¿sigue siendo buena en otro?

- Arma una matriz **meses × variantes**, parte los meses en **16 bloques** y prueba todas las formas
  de elegir 8 para «entrenar» y dejar 8 para «probar»: **12.870 combinaciones**.
- En cada una, elige la mejor variante del entrenamiento y mira en qué puesto queda en la prueba.
- **PBO** = la parte de las combinaciones en que esa «mejor» quedó en la **mitad de abajo** (o justo
  en la mediana) al probarla.

Cómo se lee: PBO cerca de 0 = elegir la mejor sirve; cerca de 0,5 = elegir la mejor es como
elegir al azar; **arriba de 0,5 = la mejor del entrenamiento tiende a ser de las peores después**.

**Nuestro caso, R040:** cuatro cruces de EMA (10, 15, 20 y 25 contra 50) en 1 h en US500, 2020–2022,
en el Laboratorio. Todas daban R positivo (+0,068 a +0,115 R por operación), pero el **PBO es
0,81** y el Sharpe deflactado va de 0,019 a 0,144. La que «gana» en un tramo no gana en el otro. El
Laboratorio calcula el PBO desde 4 variantes de una familia.

**5. Las defensas del proyecto, juntas:** el N que nunca se reinicia, el Sharpe deflactado, el
PBO, el presupuesto global de pruebas (nivel 10), las mitades, los costos × 1,5, el fuera de
muestra intocable (lección 9.4) y Harvey, Liu y Zhu (2016) recordando que, con tantas pruebas
hechas en el mundo, un t de 2 ya no alcanza.

> **Probá antes de seguir:** probaste 50 variantes de una regla y la mejor tiene un Sharpe anual de
> 0,8 en 12 años. ¿Es una buena noticia?
>
> <details><summary>Ver la respuesta</summary>
>
> No necesariamente. Con 50 pruebas sin ventaja, la mejor ya muestra un Sharpe de ese orden por
> suerte (la tabla da 0,71 anual con 100 pruebas y 0,44 con 10). Hace falta el Sharpe deflactado con
> el N verdadero (las 50 más todo lo probado antes) y el PBO de la familia.
>
> </details>

## Interactivo

La matriz meses × variantes de una familia del Laboratorio con su PBO y el resumen de los logits
(`research::pbo`), y el Sharpe deflactado de cada variante con su N.

## Ejercicio

Probá 20 variantes al azar de una regla sin ventaja (en papel: tirá una moneda 20 veces por
variante y contá las caras). ¿Alguna «ganó» claramente? ¿La elegirías si no supieras que es azar?

<details><summary>Ver la solución</summary>

Casi siempre alguna se separa: con 20 variantes, la mejor queda bastante arriba del promedio por
pura suerte. Si solo vieras esa, parecería una regla con ventaja. Por eso se cuentan **todas** las
variantes probadas y se usa el Sharpe deflactado y el PBO.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **PBO y Sharpe deflactado** de los informes: la probabilidad de sobreajuste de la familia, sus
  logits y el Sharpe deflactado de cada variante con su N.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Fórmula del Sharpe deflactado en el motor | `pub fn deflated_sharpe(xs: &[f64], trials: u32) -> Option<Deflated>` | `engine/crates/backtest/src/research/stats.rs` | A |
| Sharpe deflactado: cita | `Journal of Portfolio Management, 40(5), 94–107` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| Sharpe deflactado mínimo | `0.95` | `config/research.toml` `rule.dsr_min` | A |
| N del proyecto hoy | `361` | `config/research.toml` `rule_program5.dsr_trials` | A |
| Base del N (pruebas de antes del registro) | `344` | `config/research.toml` `alpha_budget.dsr_trials_base` | A |
| Nuestros Sharpe deflactados | `El Sharpe deflactado de nuestras pruebas va de 0,000 a 0,036` | `docs/analysis/2026-11-autoanalisis.md` | A |
| Comprar y mantener: Sharpe mensual (EUR) | `+1.0955 % (0.2607)` | `docs/analysis/registro-de-resultados.csv` `R018` | A |
| Comprar y mantener: Sharpe mensual (USD) | `Sharpe mensual 0.237` | `docs/analysis/registro-de-resultados.csv` `R018` | A |
| Bloques del PBO | 16 (`16`) | `config/research.toml` `pbo.partitions` | A |
| Combinaciones del PBO | `C(16, 8) = 12.870 combinaciones` | `config/research.toml` | A |
| PBO = mejor del entrenamiento en la mitad de abajo | `PBO = P(λ ≤ 0)` | `engine/crates/backtest/src/research/pbo.rs` | A |
| R040: R, Sharpe deflactado y PBO | `+0.068 a +0.115 R; DSR 0.019 a 0.144; PBO 0.81` | `docs/analysis/registro-de-resultados.csv` `R040` | A |
| R040: N del Sharpe deflactado | `Sharpe deflactado con N 365` | `docs/analysis/registro-de-resultados.csv` `R040` | A |
| PBO del Laboratorio desde 4 variantes | `PBO por CSCV desde 4 variantes` | `docs/modules/lab.md` | A |
| PBO por CSCV | — | [Bailey, Borwein, López de Prado y Zhu (2017), *The Probability of Backtest Overfitting*, Journal of Computational Finance 20(4), 39–69](https://www.risk.net/journal-of-computational-finance/2471206/the-probability-of-backtest-overfitting), resumen leído el 26-11-2026 | A |
| t > 3 para un factor nuevo | — | [Harvey, Liu y Zhu (2016), *…and the Cross-Section of Expected Returns*, Review of Financial Studies 29(1)](https://www.nber.org/papers/w20592), resumen leído el 26-11-2026 | A |
