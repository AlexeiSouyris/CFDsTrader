# 6.2 Medias (SMA, EMA) y cruces

> **Nivel 6 · Indicadores.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Calcular una [SMA](../GLOSARIO.md#sma) y una [EMA](../GLOSARIO.md#ema) como las calcula el motor,
entender por qué siempre llegan tarde y leer un
[cruce dorado o de la muerte](../GLOSARIO.md#cruce-dorado) sin creer que es una predicción.

## La idea

**SMA(n):** el promedio de los últimos `n` cierres. El primer valor sale con la vela `n`.

**EMA(n):** un promedio que pesa más lo reciente.

- α = 2 ÷ (n + 1). Con n = 9, α = 0,2; con n = 3, α = 0,5.
- **Semilla:** el primer valor es la SMA(n) de las primeras `n` velas.
- Después: EMA nueva = EMA anterior + α × (cierre − EMA anterior).

Es la misma semilla y el mismo α que el `ta.ema` de TradingView, y hay un test que lo compara con la
fórmula publicada desde la primera vela (lección 6.5). Como el ATR, las dos se calculan **solo con
velas cerradas** y **no repintan**.

**El retraso.** Una media resume el pasado: siempre llega tarde. Si el precio sube 1 punto por vela,
la SMA(n) queda exactamente donde estaba el precio hace (n − 1) ÷ 2 velas. La EMA le da más peso
al último cierre (α en vez de 1 ÷ n), así que se mueve antes después de un salto; pero con una suba
pareja llega igual de tarde: también (n − 1) ÷ 2 velas, una vez que se olvidó la semilla.

**Dónde las usa el motor.**

- **Tendencia de 1 h** (la de la estrategia en uso, lección 4.3): el cierre frente a la SMA de 50
  velas de 1 h y la pendiente de esa SMA contra 5 velas atrás.
- **EMA 365 diaria** en el gráfico: cada vela de 15 m muestra el valor del último día **ya
  terminado**. Es solo para mirar.
- **Cruces de medias** en el gráfico: SMA 50 y SMA 200 de los cierres **diarios**. Cruce dorado = la
  rápida pasa por encima de la lenta; cruce de la muerte = por debajo. Un toque (las dos iguales)
  que vuelve al mismo lado **no** es cruce. También es solo para mirar.

**Lo que no dicen.** Un cruce dorado llega cuando la suba ya pasó por las dos medias: es una
descripción tardía, no una predicción. En nuestro laboratorio, cuatro cruces de EMA rápidas contra
la EMA 50 en 1 h dieron entre +0,068 y +0,115 R netos por operación en 2020–2022, pero con una
**probabilidad de sobreajuste (PBO) de 0,81**: lo más probable es que la "mejor" variante sea
suerte de haber elegido entre varias (R040, exploratorio, no vota). El nivel 9 explica el PBO.

> **Probá antes de seguir:** cierres 100, 101, 102, 103 y 104. ¿Cuánto vale la SMA(5) en la última
> vela, y a qué cierre se parece?
>
> <details><summary>Ver la respuesta</summary>
>
> (100 + 101 + 102 + 103 + 104) ÷ 5 = 102: el cierre de hace 2 velas, o sea (5 − 1) ÷ 2. Mientras
> el precio sube parejo, la media va 2 velas atrás.
>
> </details>

## Interactivo

Las capas de medias y de cruces del menú Herramientas: SMA y EMA con su período, la EMA 365 diaria
y los cruces de la SMA 50 y la SMA 200 diarias, todo calculado por el motor.

## Ejercicio

Cierres: 100, 102, 104, 103 y 105. Calculá la SMA(3) y la EMA(3) desde la tercera vela.

<details><summary>Ver la solución</summary>

- **SMA(3):** 102 (100, 102, 104), 103 (102, 104, 103) y 104 (104, 103, 105).
- **EMA(3):** α = 2 ÷ 4 = 0,5. Semilla en la tercera vela: la SMA(3), 102. Cuarta: 102 + 0,5 ×
  (103 − 102) = 102,5. Quinta: 102,5 + 0,5 × (105 − 102,5) = 103,75.

En la quinta vela la EMA se movió la mitad de la distancia hasta el cierre nuevo (α = 0,5); la
SMA cambió por el cierre que entró (105) y el que salió (102). Con tan pocas velas no hay una
"ganadora": las dos resumen el pasado.

</details>

**Para hacer en la Terminal (sin nota):** con la capa de cruces, contá en el gráfico diario del
US500 cuántos cruces dorados hubo en 10 años y en cuántos el cierre de 3 meses después estaba más
abajo que el del día del cruce. No hay una clave: depende de los datos. Anotalo en el Diario.

## Examen

7 preguntas (cálculos y conceptos). Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **EMA, SMA y cruces**: las capas de medias móviles, la EMA 365 diaria y los cruces dorados y de la
  muerte.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| α de la EMA | `α = 2 / (n + 1)` | `engine/crates/indicators/src/moving_average.rs` | A |
| EMA: fórmula | `EMA_t = EMA_{t−1} + α · (C_t − EMA_{t−1})` | `engine/crates/indicators/src/moving_average.rs` | A |
| EMA sembrada con la SMA (test) | `ema_is_seeded_with_the_sma_and_uses_alpha_two_over_n_plus_one` | `engine/crates/indicators/src/moving_average.rs` | A |
| Sin repintado (test) | `values_never_change_when_new_candles_arrive` | `engine/crates/indicators/src/moving_average.rs` | A |
| Tendencia de 1 h: SMA de 50 velas | 50 (`50`) | `config/indicators.toml` `trend.sma_period` | A |
| Pendiente contra 5 velas atrás | 5 (`5`) | `config/indicators.toml` `trend.slope_lookback` | A |
| EMA diaria del gráfico | 365 (`365`) | `config/analysis.toml` `ema_daily.period` | A |
| Cruces: media rápida | SMA 50 (`50`) | `config/analysis.toml` `ma_cross.fast` | A |
| Cruces: media lenta | SMA 200 (`200`) | `config/analysis.toml` `ma_cross.slow` | A |
| Cruces: tipo de media | `sma` | `config/analysis.toml` `ma_cross.kind` | A |
| Un toque que vuelve no es cruce | `que vuelve al mismo lado no es cruce` | `engine/crates/indicators/src/cross.rs` | A |
| Las medias y los cruces solo se muestran | `NO entran al puntaje` | `config/analysis.toml` | A |
| Cruces de EMA en el laboratorio: R neto, PBO | `+0.068 a +0.115 R; DSR 0.019 a 0.144; PBO 0.81` | `docs/analysis/registro-de-resultados.csv` `R040` | A |
| Contraste de SMA y EMA con TradingView (fórmula publicada) | `pine_ema10` | `engine/crates/indicators/tests/reference.rs` | A |
