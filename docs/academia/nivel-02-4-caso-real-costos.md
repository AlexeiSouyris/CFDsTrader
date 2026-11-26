# 2.4 Caso real: los costos mataron la estrategia

> **Nivel 2 · Costos reales.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del pasado, con
> datos de futuros y costos del CFD: no dicen nada seguro del futuro.

## Objetivo

Leer un resultado propio en tres columnas: **antes de costos**, **costos** y **neto**, y ver por
qué una idea que "casi empata" termina perdiendo.

## La idea

Nuestro [registro de resultados](../analysis/registro-de-resultados.md) guarda cada prueba
con su número (R001, R002...). Tres casos muestran lo mismo desde ángulos distintos.

**Caso 1: la estrategia en uso (R001 a R003).** La estrategia de 15 minutos, probada con los futuros
de 2011-02 a 2025-11, en [R](../GLOSARIO.md#r-multiplo) por operación:

| Índice | Antes de costos | Costos | Neto | Fila |
|---|---|---|---|---|
| US500 | −0,02 R | 0,18 R | −0,19 R | R001 |
| US100 | +0,01 R | 0,13 R | −0,12 R | R002 |
| US30 | −0,05 R | 0,10 R | −0,16 R | R003 |

(El neto no es exactamente la resta por redondeo.) **Antes de costos no hay ventaja**: el resultado
está cerca de cero. Los costos hacen la pérdida. En US500, de 56.162 € de costos, 43.528 € fueron
spread (77,5 %), 9.672 € slippage y 2.961 € financiación.

**Caso 2: el momentum de la última media hora (R017 y R026).** La cartera perdió un 2,0 % por mes
con un t de −11: parece un efecto enorme. Pero **antes de costos** daba −0,010, −0,013 y −0,031 R
por operación, casi cero. El t de −11 venía de restar un costo casi fijo, de 0,08 a 0,15 R, en unas
2.450 operaciones por índice. Con tantas operaciones, un costo chico y constante da un t muy grande.

**Caso 3: la deriva nocturna (R021).** Entre las 02:00 y las 03:00 de Nueva York los índices suben
un poquito en promedio (+0,83 puntos básicos por día en US500, con un t de 3,15): el efecto **es
real** en nuestros datos. Pero abrir y cerrar el CFD a esa hora cuesta **2 a 4 veces más** que lo
que da. Existe y no se puede aprovechar con este vehículo.

**La lección:** antes de preguntar "¿esto funciona?", preguntá "¿funciona **después de costos**?".
Un costo de 0,1 R por operación convierte una idea que empata en una que pierde 10 R cada 100
operaciones.

> **Probá antes de seguir:** desde 2020 el costo por operación de la estrategia bajó a menos de la
> mitad. ¿Por qué podría ser?
>
> <details><summary>Ver la respuesta</summary>
>
> El spread está fijo en puntos y el índice cotiza mucho más alto que en 2011, así que el mismo
> spread pesa menos frente a un stop medido en ATR. Además, el spread real de IG de 2011 no se
> conoce: parte del cambio puede ser un artefacto del supuesto.
>
> </details>

## Interactivo

La tabla del registro con las columnas "antes de costos / costos / neto" de cada caso, con un
enlace a su informe.

## Ejercicio

Ordená de mayor a menor cuánto les come el costo a estas tres estrategias, en R por operación:
la estrategia en uso en US500 (R001), el momentum de la última media hora en US500 (R026, costo
medio 0,149 R) y la misma estrategia en uso en US30 (R003).

<details><summary>Ver la solución</summary>

R001 US500 (0,18 R) > momentum US500 (0,149 R) > R003 US30 (0,10 R). En las tres, el costo es mucho
más grande que lo que la idea gana antes de costos.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Calculadora de costos**: el ticket muestra el costo previsto (spread, slippage y financiación)
  antes de abrir.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| US500: neto, antes de costos y costos | −0,19 R (`-0.19 (-0.02 / 0.18)`) | `docs/analysis/registro-de-resultados.csv` `R001` | A |
| US100: neto, antes de costos y costos | −0,12 R (`-0.12 (+0.01 / 0.13)`) | `docs/analysis/registro-de-resultados.csv` `R002` | A |
| US30: neto, antes de costos y costos | −0,16 R (`-0.16 (-0.05 / 0.10)`) | `docs/analysis/registro-de-resultados.csv` `R003` | A |
| US500: costos totales y spread | 56.162 € y 43.528 € (`43 528`) | `docs/analysis/2026-11-diagnostico-us-futuros.md` (tabla "Costos: antes y después") | A |
| US500: total de costos | `56 162` | `docs/analysis/2026-11-diagnostico-us-futuros.md` | A |
| US500: slippage y financiación | `9672` | `docs/analysis/2026-11-diagnostico-us-futuros.md` | A |
| Momentum última media hora: −2,0 % por mes, t −11 | `-2.0 % (t -11.0` | `docs/analysis/registro-de-resultados.csv` `R017` | A |
| Momentum antes de costos (US500 / US100 / US30) | `-0.010 / -0.013 / -0.031 R` | `docs/analysis/registro-de-resultados.csv` `R026` | A |
| Costo fijo por operación del momentum | `0.08 a 0.15 R` | `docs/analysis/registro-de-resultados.csv` `R026` | A |
| Operaciones del momentum por índice | unas 2.450 (`2 450 operaciones por índice`) | `docs/analysis/2026-11-chequeos-revision-externa.md` | A |
| Costo medio del momentum en US500 | 0,149 R (`0,149 R`) | `docs/analysis/2026-11-chequeos-revision-externa.md` | A |
| Deriva nocturna US500 (+0,83 pb, t 3,15) | `+0.83 / +1.03 / +0.63 pb (t 3.15)` | `docs/analysis/registro-de-resultados.csv` `R021` | A |
| Costo del CFD frente a la deriva | `2 a 4 veces mayor` | `docs/analysis/registro-de-resultados.csv` `R021` | A |
| El costo por operación bajó a menos de la mitad desde 2020 | `bajan a menos de la mitad desde 2020` | `docs/analysis/2026-11-diagnostico-us-futuros.md` | A |
| Costos de transacción en general | — | [Harris, *Trading and Exchanges*, Oxford University Press (2002/2003)](https://ideas.repec.org/b/oxp/obooks/9780195144703.html) | B |
