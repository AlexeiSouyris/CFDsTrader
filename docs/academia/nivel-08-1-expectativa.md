# 8.1 Expectativa

> **Nivel 8 · Expectativa, varianza y suerte.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los números
> de los ejemplos son redondos, de práctica: no son resultados de ninguna estrategia.

## Objetivo

Calcular la **expectativa** por operación en [R](../GLOSARIO.md#r-multiplo), **después de
costos**, y el porcentaje de aciertos mínimo para no perder.

## La idea

Una operación sola no dice nada: puede ganar o perder por suerte. Lo que importa es qué pasa **en
promedio**, si repetís la misma regla muchas veces. Eso es la expectativa:

E = p × G − (1 − p) × P

- **p** = la parte de operaciones que ganan (el [win rate](../GLOSARIO.md#win-rate)).
- **G** = lo que gana en promedio una ganadora, en R.
- **P** = lo que pierde en promedio una perdedora, en R (en positivo; 1 = tocó el stop).

Con 40 % de aciertos, ganadoras de 2 R y perdedoras de 1 R: E = 0,4 × 2 − 0,6 × 1 = **+0,2 R**
por operación, antes de costos.

**1. El win rate solo no dice nada.** Una regla que acierta el 70 % pero gana 0,4 R y pierde 1 R
tiene E = 0,7 × 0,4 − 0,3 × 1 = **−0,02 R**: pierde aunque gane 7 de cada 10. Y una que acierta
el 35 % con ganadoras de 3 R tiene E = 0,35 × 3 − 0,65 = +0,4 R. Siempre se miran las dos cosas
juntas.

**2. Los costos van adentro.** Cada operación paga spread, slippage y, si duerme, financiación
(nivel 2). Si el costo es **c** R por operación, cada ganadora gana G − c y cada perdedora pierde
P + c, así que:

E neta = p × G − (1 − p) × P − c

Con la regla de arriba (+0,2 R) y 0,18 R de costos, como la estrategia en uso en US500: **+0,02 R**.
Casi todo lo que parecía ventaja se lo come el costo.

**3. El porcentaje mínimo de aciertos.** E neta = 0 cuando:

p mínimo = (P + c) ÷ (G + P)

- Objetivo de 1,5 R y stop de 1 R, sin costos: 1 ÷ 2,5 = **40 %**.
- Con 0,13 R de costos (US100): 1,13 ÷ 2,5 = **45,2 %**. El costo sube la vara más de 5 puntos.

**4. Qué nos pasó a nosotros.** La estrategia en uso (15 minutos, futuros con los costos del CFD)
queda **cerca de cero antes de costos** y pierde por los costos:

| Índice | R antes de costos | Costos | R neto por operación |
|---|---|---|---|
| US500 (R001) | −0,02 | 0,18 | −0,19 |
| US100 (R002) | +0,01 | 0,13 | −0,12 |
| US30 (R003) | −0,05 | 0,10 | −0,16 |

(El neto puede no dar la resta exacta por redondeo.) Y H4.2, la regla de Bruni normalizada
(programa 4), ganaba +0,069 R antes de costos, pero el costo se comía 0,11 R: con stops cortos, el
mismo spread en puntos es una parte más grande de 1 R.

**5. Dónde lo calcula el motor.** El simulador de varianza del Laboratorio usa esta misma fórmula
(`research::variance`), y cada informe del Laboratorio muestra la expectativa como **R medio neto
con su intervalo de confianza**, el win rate con su intervalo de Wilson y el
[profit factor](../GLOSARIO.md#profit-factor). El intervalo importa: la próxima lección y la 8.4
muestran cuánto puede moverse un promedio por pura suerte.

> **Probá antes de seguir:** objetivo de 2 R, stop de 1 R y 0,1 R de costos. ¿Qué win rate
> necesitás como mínimo?
>
> <details><summary>Ver la respuesta</summary>
>
> (1 + 0,1) ÷ (2 + 1) = 1,1 ÷ 3 ≈ **36,7 %**. Sin costos alcanzaría con 33,3 %.
>
> </details>

## Interactivo

Deslizadores de p, G y P: el motor calcula E con la fórmula del simulador de varianza (se abre en
la lección 8.3) y la muestra con y sin el costo de tu instrumento. Al lado, la tabla de R001 a R003
con antes de costos, costos y neto.

## Ejercicio

Cinco casos. Calculá E neta y decí si gana o pierde:

1. p 50 %, G 1 R, P 1 R, c 0,05 R.
2. p 30 %, G 3 R, P 1 R, c 0,15 R.
3. p 60 %, G 0,8 R, P 1 R, c 0.
4. p 45 %, G 1,5 R, P 1 R, c 0,13 R.
5. Objetivo 1,5 R, stop 1 R, sin costos: ¿qué p mínimo?

<details><summary>Ver la solución</summary>

1. 0,5 − 0,5 − 0,05 = **−0,05 R**: pierde (sin ventaja, el costo decide).
2. 0,9 − 0,7 − 0,15 = **+0,05 R**: gana poco.
3. 0,48 − 0,4 = **+0,08 R**: gana.
4. 0,675 − 0,55 − 0,13 = **−0,005 R**: pierde por muy poco (45 % está debajo del 45,2 % mínimo).
5. 1 ÷ 2,5 = **40 %**.

</details>

## Examen

6 preguntas (cálculos y conceptos). Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo: el simulador de varianza, que calcula E por vos, se abre en la lección 8.3.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Fórmula de la expectativa en el motor | `self.win_rate * self.avg_win_r - (1.0 - self.win_rate) * self.avg_loss_r` | `engine/crates/backtest/src/research/variance.rs` | A |
| US500: neto (antes de costos / costos) | `-0.19 (-0.02 / 0.18)` | `docs/analysis/registro-de-resultados.csv` `R001` | A |
| US100: neto (antes de costos / costos) | `-0.12 (+0.01 / 0.13)` | `docs/analysis/registro-de-resultados.csv` `R002` | A |
| US30: neto (antes de costos / costos) | `-0.16 (-0.05 / 0.10)` | `docs/analysis/registro-de-resultados.csv` `R003` | A |
| Ventaja bruta y costo de nuestras pruebas | `ventaja bruta ≈ 0 y costo de 0,10 a 0,18 R` | `docs/analysis/2026-11-autoanalisis.md` | A |
| H4.2 antes de costos | `Antes de costos ganaba algo (+0,069 R)` | `docs/analysis/2026-11-programa-4-h41-h42-resultados.md` | A |
| H4.2: el costo en R | `el costo del CFD se come 0,11 R por operación` | `docs/analysis/2026-11-programa-4-h41-h42-resultados.md` | A |
| Qué muestra el informe del Laboratorio | `R medio neto con IC; win rate con IC de Wilson; profit factor` | `docs/design/2026-11-laboratorio.md` | A |
| Nivel de los intervalos del informe | 95 % (`0.95`) | `config/lab.toml` `report.confidence` | A |
