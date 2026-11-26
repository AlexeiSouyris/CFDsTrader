# 6.4 Herramientas que no puntúan

> **Nivel 6 · Indicadores.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Saber qué dibujan [Fibonacci](../GLOSARIO.md#fibonacci), el
[golden pocket](../GLOSARIO.md#golden-pocket), los [puntos pivote diarios](../GLOSARIO.md#puntos-pivote),
el [spring y el upthrust](../GLOSARIO.md#spring), el [precio de fondo](../GLOSARIO.md#precio-de-fondo)
y el régimen del [VIX](../GLOSARIO.md#vix), con la regla exacta del motor, y por qué **se muestran
pero no entran al puntaje**.

## La idea

El menú Herramientas tiene capas que muchos traders miran. El motor las calcula con una regla
medible, pero son **solo para mirar**: no suman puntos, no las usa la estrategia en uso y no tocan
el riesgo. En nuestro registro ninguna mostró que agregue ventaja; la única que probamos, el VIX
como variable del modelo de ML, **no mejoró** nada (R012: AUC 0,508, +0,0008 frente a la
referencia).

**Fibonacci y golden pocket.** Sobre el último **tramo** entre dos [pivotes](../GLOSARIO.md#pivote)
confirmados (de A a B):

- **Retrocesos** (vuelve hacia A): precio = B − r × (B − A), con r = 0,236, 0,382, 0,5, 0,618 y
  0,786.
- **Extensiones** (sigue más allá de B): precio = A + e × (B − A), con e = 1,272, 1,618 y 2,618.
- **Golden pocket:** la zona entre los retrocesos 0,618 y 0,65.

Casi todos salen del número de oro φ ≈ 1,618 (0,618 ≈ 1 ÷ φ). El tramo existe recién cuando B se
confirma, así que no mira el futuro.

**Puntos pivote diarios (clásicos).** Con el máximo H, el mínimo L y el cierre C del **último día
terminado**: P = (H + L + C) ÷ 3, R1 = 2P − L, S1 = 2P − H (y R2, S2, R3, S3). Valen todo el día
siguiente; nunca se calculan con el día en curso. No confundir con el pivote del nivel 4 (un máximo o
mínimo local confirmado).

**Spring y upthrust.** Un rango con un soporte y una resistencia de **al menos 3 toques** dentro de
**±0,15 %**, en las últimas 200 velas. **Spring:** el mínimo de una vela pasa el soporte (más allá
de esa tolerancia) y el precio **vuelve a cerrar por encima del soporte** en **3 velas** o menos (0 =
la misma vela); si el mínimo pasa el soporte por más de **0,5 %**, es una ruptura de verdad y no un
spring. La señal va en la vela que cerró de vuelta adentro. **Upthrust:** el espejo, arriba.

**Precio de fondo.** Un pivote bajo confirmado que cierra una bajada de al menos **1 %** o **3
ATR** (alcanza con una) desde el máximo de las 50 velas anteriores, y desde el que algún cierre
rebota **1 ATR** en **10 velas** o menos.

**Régimen del VIX.** Contexto, no dirección: calma por debajo de **15**, normal de 15 a 20, elevado
de 20 a 30 y estrés desde **30**. Es la convención habitual del mercado, no un ajuste nuestro.

> **Probá antes de seguir:** el último tramo subió de A = 6.000 a B = 6.100. ¿Dónde está el
> retroceso 0,5?
>
> <details><summary>Ver la respuesta</summary>
>
> 6.100 − 0,5 × 100 = **6.050**. El 0,5 no sale de φ: es costumbre.
>
> </details>

## Interactivo

El menú Herramientas completo sobre un gráfico real: cada capa se prende y se apaga, y todas las
calcula el motor.

## Ejemplo resuelto

Ayer: H = 6.120, L = 6.000, C = 6.090.

- P = (6.120 + 6.000 + 6.090) ÷ 3 = **6.070**.
- R1 = 2 × 6.070 − 6.000 = **6.140**.
- S1 = 2 × 6.070 − 6.120 = **6.020**.

Son los mismos números para todos los que usan la fórmula clásica: por eso mucha gente los mira. Eso
no prueba que el precio "respete" esos niveles.

## Ejercicio

El soporte de un rango está en 6.000. Tres casos: (a) una vela baja a 5.985 y la siguiente cierra
en 6.004; (b) el precio baja a 5.960 y vuelve a cerrar en 6.002 dos velas después; (c) baja a 5.980
y recién cierra por encima de 6.000 cinco velas después. ¿Cuál es un spring para el motor?

<details><summary>Ver la solución</summary>

Solo (a). En (b) pasó el soporte un 0,67 % (más de 0,5 %): es una ruptura, no un spring. En (c)
tardó 5 velas en volver (más de 3).

</details>

## Examen

7 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Fibonacci y golden pocket**.
- **Puntos pivote diarios**.
- **Spring y upthrust**.
- **Precios de fondo**.
- **Régimen del VIX**.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Solo se muestran: no entran al puntaje | `NO entran al puntaje` | `config/analysis.toml` | A |
| Retrocesos de Fibonacci | `retracements = ["0.236", "0.382", "0.5", "0.618", "0.786"]` | `config/analysis.toml` | A |
| Extensiones de Fibonacci | `extensions = ["1.272", "1.618", "2.618"]` | `config/analysis.toml` | A |
| Golden pocket | `golden_pocket = ["0.618", "0.65"]` | `config/analysis.toml` | A |
| Retroceso: precio | `precio = B − r · (B − A)` | `engine/crates/patterns/src/fibonacci.rs` | A |
| Pivote diario | `P  = (H + L + C) / 3` | `engine/crates/indicators/src/floor_pivots.rs` | A |
| R1 | `R1 = 2P − L` | `engine/crates/indicators/src/floor_pivots.rs` | A |
| S1 | `S1 = 2P − H` | `engine/crates/indicators/src/floor_pivots.rs` | A |
| Spring: toques mínimos del rango | 3 (`3`) | `config/analysis.toml` `wyckoff.min_touches` | A |
| Spring: tolerancia del rango | ±0,15 % (`0.15`) | `config/analysis.toml` `wyckoff.tolerance_pct` | A |
| Spring: velas de búsqueda del rango | 200 (`200`) | `config/analysis.toml` `wyckoff.lookback_bars` | A |
| Spring: velas para volver a cerrar dentro | 3 (`3`) | `config/analysis.toml` `wyckoff.max_bars_to_reclaim` | A |
| Spring: más de 0,5 % es una ruptura | 0,5 % (`0.5`) | `config/analysis.toml` `wyckoff.max_penetration_pct` | A |
| Fondo: caída mínima en % | 1 % (`1`) | `config/analysis.toml` `bottom.min_decline_pct` | A |
| Fondo: caída mínima en ATR | 3 (`3`) | `config/analysis.toml` `bottom.min_decline_atr` | A |
| Fondo: velas para el máximo previo | 50 (`50`) | `config/analysis.toml` `bottom.lookback` | A |
| Fondo: rebote mínimo | 1 ATR (`1`) | `config/analysis.toml` `bottom.min_rebound_atr` | A |
| Fondo: velas para el rebote | 10 (`10`) | `config/analysis.toml` `bottom.max_bars_to_rebound` | A |
| VIX: desde dónde es normal | 15 (`15`) | `config/analysis.toml` `vix.normal_from` | A |
| VIX: desde dónde es elevado | 20 (`20`) | `config/analysis.toml` `vix.elevated_from` | A |
| VIX: desde dónde es estrés | 30 (`30`) | `config/analysis.toml` `vix.stress_from` | A |
| VIX como variable de ML: no mejora | `0.508 (+0.0008)` | `docs/analysis/registro-de-resultados.csv` `R012` | A |
