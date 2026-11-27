# 2.3 Slippage y comisión

> **Nivel 2 · Costos reales.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Saber que **el precio de la orden no es el precio que te dan**: el
[slippage](../GLOSARIO.md#slippage), los [gaps](../GLOSARIO.md#gap) y la
[comisión](../GLOSARIO.md#comision) de las acciones.

## La idea

**Slippage.** Entre que decidís y que la orden se ejecuta, el mercado se mueve. Casi siempre te
ejecutan un poco peor de lo que veías. Nuestro motor nunca lo pone en cero: supone, **en cada
ejecución** (entrada y salida), un slippage de

```text
slippage = 0,1 × spread + 1 tick        siempre en contra
```

**La vela siguiente.** Una señal que aparece al cierre de la vela `t` se ejecuta en la **apertura
de la vela `t+1`**, nunca en el cierre de `t`: en el momento de decidir, ese cierre recién se
conoce. Así trabajan el backtest y el paper, para no mirar el futuro.

**Gaps.** Cuando el precio salta sin operar en el medio (fin de semana, noticias fuertes), un stop
se ejecuta **donde reabre** el mercado, no donde lo pusiste. Por eso el tamaño se calcula con un
colchón de 0,25 ATR además de la distancia al stop.

**Comisión.** Los CFD de índices de IG no tienen comisión: el costo es el spread. Los CFD de
**acciones** no tienen spread de IG pero pagan comisión **al abrir y al cerrar**:

| Acciones | Comisión por ejecución | Mínimo |
|---|---|---|
| EE. UU. | 0,02 USD por acción | 15 USD |
| Europa | 0,06 % del nominal | 5 € |

El mínimo pesa mucho en operaciones chicas: 100 acciones de EE. UU. pagan 15 USD al abrir y otros
15 al cerrar, aunque 100 × 0,02 sean solo 2 USD.

> **Probá antes de seguir:** ves un cruce en la vela que cierra a las 10:15. ¿A qué precio entrás?
>
> <details><summary>Ver la respuesta</summary>
>
> Al de la apertura de la vela siguiente (10:15 en adelante), más medio spread y el slippage. El
> cierre de las 10:15 ya pasó cuando lo ves.
>
> </details>

## Interactivo

El replay de un día con una noticia fuerte: la orden se marca en una vela y se ejecuta en la
siguiente, y se ve cuánto se movió el precio en el medio.

## Ejemplo resuelto

Slippage por ejecución y costo de ida y vuelta (spread + 2 ejecuciones de slippage):

| Instrumento | Spread | Tick | Slippage por ejecución | Ida y vuelta |
|---|---|---|---|---|
| US500 | 0,9 | 0,01 | 0,1 × 0,9 + 0,01 = 0,10 | 0,9 + 2 × 0,10 = **1,1 puntos** |
| US100 | 1,6 | 0,1 | 0,1 × 1,6 + 0,1 = 0,26 | 1,6 + 2 × 0,26 = **2,12 puntos** |
| US30 | 3,7 | 0,1 | 0,1 × 3,7 + 0,1 = 0,47 | 3,7 + 2 × 0,47 = **4,64 puntos** |
| DE40 | 1,8 | 0,1 | 0,1 × 1,8 + 0,1 = 0,28 | 1,8 + 2 × 0,28 = **2,36 puntos** |

Con un stop de 8 puntos en US500, los 1,1 puntos son unos 0,14 R por operación, antes de la
financiación.

## Ejercicio

Calculá la comisión de ida y vuelta (abrir y cerrar) de:

1. 500 acciones de EE. UU.
2. Una posición de 8.000 € en acciones europeas (mismo nominal al cerrar).

<details><summary>Ver la solución</summary>

1. 500 × 0,02 = 10 USD, menos que el mínimo: 15 USD al abrir + 15 al cerrar = **30 USD**.
2. 8.000 × 0,06 % = 4,80 €, menos que el mínimo: 5 € + 5 € = **10 €**.

</details>

## Examen

7 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Slippage: fracción del spread por ejecución | 0,1 (`0.1`) | `config/backtest.toml` `costs.slippage_spread_fraction` (spec §11) | A |
| Slippage: ticks por ejecución | 1 (`1`) | `config/backtest.toml` `costs.slippage_ticks` | A |
| Tick US500 | 0,01 (`0.01`) | `config/instruments.toml` `instruments.US500.tick` (API de IG) | A |
| Tick US100 | 0,1 (`0.1`) | `config/instruments.toml` `instruments.US100.tick` | A |
| Tick US30 | 0,1 (`0.1`) | `config/instruments.toml` `instruments.US30.tick` | A |
| Tick DE40 | 0,1 (`0.1`) | `config/instruments.toml` `instruments.DE40.tick` | A |
| Spread típico US500 | 0,9 (`0.9`) | `config/instruments.toml` `instruments.US500.spread_typical_points` | A |
| Spread típico US100 | 1,6 (`1.6`) | `config/instruments.toml` `instruments.US100.spread_typical_points` | A |
| Costo de ida y vuelta en US500 | 1,1 puntos (`1,1 puntos de costo por ida y vuelta`) | `docs/analysis/2026-11-propuesta-investigacion.md` | A |
| Comisión de acciones de EE. UU. por acción | 0,02 USD (`0.02`) | `config/instruments.toml` `share_costs.US.commission_per_share` (ig.com/es) | A |
| Comisión mínima de acciones de EE. UU. | 15 USD (`15`) | `config/instruments.toml` `share_costs.US.commission_min` | A |
| Comisión de acciones europeas | 0,06 % (`0.06`) | `config/instruments.toml` `share_costs.EU.commission_pct` | A |
| Comisión mínima de acciones europeas | 5 € (`5`) | `config/instruments.toml` `share_costs.EU.commission_min` | A |
| Colchón por gaps | 0,25 ATR (`0.25`) | `config/risk.toml` `gap.gap_atr` | A |
| Decidir al cierre de t, ejecutar en t+1 | `se ejecuta en` | `CLAUDE.md` §5b | A |
| Ejemplo del gap (stop en 26.600 ejecutado en 26.400) | `se ejecuta en 26.400` | `docs/GLOSARIO.md` (Gap) | A |
| Comisión de acciones (2 centavos, mínimo 15 USD; 0,06 %, mínimo 5 €) | — | [IG España, "Nuestros costes"](https://www.ig.com/es/nuestros-costes), leído el 26-11-2026 | A |
| Costos de ejecución y slippage | — | [Harris, *Trading and Exchanges*, Oxford University Press (2002/2003)](https://ideas.repec.org/b/oxp/obooks/9780195144703.html) | B |
