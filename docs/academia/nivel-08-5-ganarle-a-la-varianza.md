# 8.5 Qué significa «ganarle a la varianza»

> **Nivel 8 · Expectativa, varianza y suerte.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son
> resultados del pasado: no dicen nada seguro del futuro.

## Objetivo

Juntar el nivel en una idea: a la varianza **no se le gana**. Se la **sobrevive** con tamaño chico
y se la **mide** con muestras grandes. Y sin ventaja después de costos, ningún manejo de riesgo la
crea.

## La idea

"Ganarle a la varianza" suena a que hay un truco para que la mala suerte no toque. No lo hay. Lo que
sí hay son tres cosas, una por lección:

**1. Una ventaja después de costos (8.1).** Si E neta ≤ 0, la varianza solo decide **cuándo** se
pierde, no **si** se pierde. Es lo que mostró R039 (nivel 7): con los topes de riesgo, la estrategia
en uso perdió −91,6 % en lugar de −97,6 %, pero perdió. Cambiar el tamaño no cambia el signo de E.

**2. Un tamaño que aguante las rachas normales (8.2 y 8.3).** Las rachas largas van a venir; la
pregunta es cuánto duelen. Por eso nuestro riesgo por operación es del **1 %**, con la reducción
gradual por caída y el kill switch al **15 %** (nivel 7). Y por eso importa cuánto cuesta volver:

| Caída | Ganancia que hace falta para volver |
|---|---|
| −10 % | +11,1 % |
| −30 % | +42,9 % |
| −50 % | +100 % |

Cuanto más honda la caída, más cuesta salir: la ganancia necesaria es caída ÷ (1 − caída). Con
tamaño grande, una racha normal te deja en un pozo del que una ventaja chica tarda años en sacarte.
El simulador de varianza deja probar hasta 10 % por operación justamente para **ver** la ruina, no
para usarla.

**3. Una muestra grande antes de creer (8.4).** Con 100 operaciones no se separa una ventaja de
0,1 R del ruido; con 1.000, sí. Y antes de confiar en una regla hay que saber cuántas se probaron
para encontrarla: eso es el nivel 9.

**Lo que no es "ganarle a la varianza":**

- Subir el tamaño después de perder para "recuperar": agranda la próxima racha.
- Cambiar de regla en cada racha: nunca se junta la muestra que dice si la regla servía.
- Creer en una curva de capital sin su abanico: un solo camino no dice cuánto pudo variar.

> **Probá antes de seguir:** una regla tiene E = +0,05 R después de costos, medida en 2.000
> operaciones. ¿Conviene arriesgar el 5 % por operación para "aprovecharla"?
>
> <details><summary>Ver la respuesta</summary>
>
> No. Con 5 % una racha de 10 pérdidas, que es normal (8.2), cuesta −40 % y pide +67 % para volver;
> una ventaja de 0,05 R tarda muchísimo en recuperarlo, y la incertidumbre del propio 0,05 R (8.4)
> puede hacer que la ventaja real sea menor. El riesgo del proyecto es del 1 %, y nunca más que eso.
>
> </details>

## Interactivo

En el simulador de varianza, la misma regla (E positiva) con 1 %, 2 % y 5 % de riesgo: el motor
muestra cómo crecen la caída máxima y la ruina con el tamaño, sin cambiar la expectativa.

## Ejercicio

Con la tabla de arriba: si una cuenta cae 25 %, ¿qué ganancia necesita para volver al máximo?

<details><summary>Ver la solución</summary>

0,25 ÷ 0,75 = **+33,3 %**.

</details>

## Examen

Esta lección no tiene examen: es el cierre del nivel. Se completa al leerla.

## Desbloquea

Nada nuevo: el simulador de varianza se abre en 8.3.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Retorno total con / sin topes | `-91.6 % / -97.6 %` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Los topes no crean ventaja | `con topes pierde menos y más despacio pero la estrategia sigue perdiendo` | `docs/analysis/registro-de-resultados.csv` `R039` | A |
| Riesgo por operación | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
| Kill switch por caída | 15 % (`15`) | `config/risk.toml` `limits.max_drawdown_pct` | A |
| Riesgo máximo que se puede simular | 10 % (`10`) | `config/lab.toml` `variance.max_risk_pct` | A |
| Por qué el simulador deja más riesgo | `sirve para VER qué pasa con riesgos altos (la ruina)` | `config/lab.toml` | A |
| El nivel en el temario | `Sin ventaja después de costos, ningún manejo de riesgo la crea (R039)` | `docs/design/2026-11-academia.md` | A |
