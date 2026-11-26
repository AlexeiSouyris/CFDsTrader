# 7.4 Límites de la cuenta

> **Nivel 7 · Riesgo y tamaño.** Educativo: no es consejo de inversión ni promete ganancias. Todo
> pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los números de los ejemplos
> son redondos.

## Objetivo

Conocer los tres frenos de la cuenta y calcular cuándo actúan: la **pérdida diaria del 3 %**, la
**reducción gradual por caída** y el [kill switch](../GLOSARIO.md#kill-switch) al **15 %**.

## La idea

Las lecciones 7.2 y 7.3 cuidan **una** operación. Estos límites cuidan **la cuenta**: frenan una
mala racha antes de que se coma todo. Ninguno cambia una señal; solo achican el tamaño o cortan las
entradas nuevas.

**1. Pérdida diaria: 3 %.** Si en el día (desde las 00:00 de Madrid, contando lo realizado y lo no
realizado) el capital cae un 3 % desde el del inicio del día, el motor **corta las entradas nuevas**
hasta el día siguiente. Con 10.000 € al empezar el día, el corte es en **9.700 €**.

**2. Reducción gradual por caída.** La caída ([drawdown](../GLOSARIO.md#drawdown)) se mide desde el
**máximo** del capital, que el motor actualiza al cierre de cada vela con las posiciones valuadas a
mercado. Según la caída, el 1 % de riesgo se multiplica por un factor:

| Caída desde el máximo | Factor | Riesgo por operación |
|---|---|---|
| hasta 5 % | 1 | 1 % |
| de 5 % a 10 % | baja en línea recta de 1 a 0,5 | de 1 % a 0,5 % |
| de 10 % a 12,5 % | baja en línea recta de 0,5 a 0,25 | de 0,5 % a 0,25 % |
| de 12,5 % hasta el kill switch | 0,25 | 0,25 % |

Ejemplo: con una caída del **7,5 %** (la mitad entre 5 y 10), el factor es **0,75** y el riesgo,
0,75 %. Con **11,25 %**, el factor es 0,375.

La idea: hasta 5 % de caída es ruido normal y no se toca nada; después, cuanto peor va, más chico se
opera. Es un freno gradual en lugar de "todo o nada".

**3. Kill switch: 15 %.** Si el capital cae un 15 % desde el máximo, el motor rechaza toda operación
nueva y quien llama **activa el kill switch**. También se activa por un error de datos, una
desconexión o una reconciliación que no cierra. En paper **no se reactiva solo**: hace falta la
decisión del dueño. (En los backtests se reactiva a los 20 días hábiles, solo para poder seguir
midiendo; en vivo, nunca.)

**Lo que estos límites no hacen.** No convierten una estrategia perdedora en ganadora: solo hacen
que pierda menos y más despacio. Lo vas a ver con números en el caso real (lección 7.5).

> **Probá antes de seguir:** el máximo del capital fue 12.000 €. ¿Con qué capital salta el kill
> switch?
>
> <details><summary>Ver la respuesta</summary>
>
> 12.000 × (1 − 0,15) = **10.200 €**. Ojo: se mide desde el máximo, no desde el capital inicial.
>
> </details>

## Interactivo

Un simulador de una mala racha: una serie de pérdidas seguidas, y el motor muestra en cada paso la
caída, el factor, el riesgo en € y cuándo saltaría el kill switch.

## Ejemplo resuelto

Capital inicial y máximo: 10.000 €. Después de varias pérdidas, el capital es **9.250 €**.

- Caída: (10.000 − 9.250) ÷ 10.000 = **7,5 %**.
- Factor: 0,75. Riesgo: 0,75 % × 9.250 = **69,38 €** por operación (frente a 100 € al principio).
- Faltan 750 € de pérdida más para llegar al kill switch (8.500 €), y cada operación arriesga cada vez
  menos.

## Ejercicio

Con un máximo de 10.000 €, decí el factor de la reducción gradual con estos capitales: 9.700,
9.400, 9.000, 8.800 y 8.600.

<details><summary>Ver la solución</summary>

- 9.700 → caída 3 %: factor **1**.
- 9.400 → 6 %: 1 − 0,5 × (6 − 5) ÷ 5 = **0,9**.
- 9.000 → 10 %: **0,5**.
- 8.800 → 12 %: 0,5 − 0,25 × (12 − 10) ÷ 2,5 = **0,3**.
- 8.600 → 14 %: **0,25** (después del último punto vale el último factor, hasta el 15 %).

</details>

## Examen

7 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Barra de estado de la operación**: el kill switch, la pérdida del día y la caída desde el
  máximo, siempre a la vista.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Pérdida diaria máxima | 3 % (`3`) | `config/risk.toml` `limits.max_daily_loss_pct` | A |
| Pérdida del día: realizada + no realizada desde las 00:00 de Madrid | `realizada + no realizada desde las 00:00 de Madrid` | `config/risk.toml` | A |
| Kill switch por caída | 15 % (`15`) | `config/risk.toml` `limits.max_drawdown_pct` | A |
| Modo de la reducción gradual | `linear` | `config/risk.toml` `drawdown_scaling.mode` | A |
| Punto 1: (5 %, 1) | `{ drawdown_pct = "5", scale = "1" }` | `config/risk.toml` | A |
| Punto 2: (10 %, 0,5) | `{ drawdown_pct = "10", scale = "0.5" }` | `config/risk.toml` | A |
| Punto 3: (12,5 %, 0,25) | `{ drawdown_pct = "12.5", scale = "0.25" }` | `config/risk.toml` | A |
| Después del último punto vale el último factor | `Después del último punto vale el último factor` | `docs/analysis/2026-11-riesgo-propuesta.md` | A |
| El máximo se actualiza al cierre de cada vela | `al cierre de **cada** vela cerrada` | `engine/crates/risk/src/lib.rs` | A |
| Causas del kill switch | `límite superado, error de datos, desconexión o reconciliación` | `engine/crates/broker/src/paper.rs` | A |
| En paper no se reactiva solo | `no se opera más hasta la decisión del dueño` | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| En backtest se reactiva a los 20 días hábiles | 20 (`20`) | `config/backtest.toml` `kill_switch.reset_after_business_days` | A |
