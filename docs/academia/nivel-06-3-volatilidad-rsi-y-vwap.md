# 6.3 Volatilidad, RSI y VWAP

> **Nivel 6 · Indicadores.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del pasado, con
> datos de futuros: no dicen nada seguro del futuro.

## Objetivo

Medir la volatilidad como la mide el motor, y saber qué miden el [RSI](../GLOSARIO.md#rsi) y el
[VWAP](../GLOSARIO.md#vwap) (que el motor **todavía no calcula**) y por qué ninguno es una señal
por sí mismo.

## La idea

**Dos formas de medir cuánto se mueve el precio.**

| | ATR (lección 6.1) | Volatilidad realizada |
|---|---|---|
| Qué usa | máximo, mínimo y cierre anterior de cada vela | solo los cierres diarios |
| Unidad | puntos por vela | fracción por día (0,0115 = 1,15 %) |
| Qué ve | el rango de cada vela y el salto desde el cierre anterior | solo el cambio de cierre a cierre, no el rango dentro del día |
| Para qué la usa el motor | stops, tamaño, tolerancias | contexto: análogos y variables de ML |

**La volatilidad realizada del motor** es el desvío estándar **muestral** de los últimos **20
retornos diarios**, con retorno = cierre ÷ cierre anterior − 1. Después, cada día se compara con
su propia historia de los **3 años** anteriores y cae en un **tercio**: baja, media o alta. Esos
tercios los usan los análogos y las variables de ML como contexto; no deciden operaciones.

**Lo que dio nuestra prueba con la volatilidad (R019).** "Tener más índice cuando el mercado está
calmo y menos cuando está agitado" dio un alfa de +0,19 % por mes contra comprar y mantener, con
p = 0,17 (hacía falta menos de 0,0125), y un Sharpe mensual **menor** (0,245 frente a 0,261). **No
pasó.** Medir bien la volatilidad no alcanza para tener ventaja.

**RSI.** Lo presentó Wilder en 1978. Con las ganancias medias y las pérdidas medias de las últimas
14 velas:

- RS = ganancia media ÷ pérdida media;
- RSI = 100 − 100 ÷ (1 + RS), entre 0 y 100.

Por encima de 70 se habla de "sobrecompra" y por debajo de 30 de "sobreventa". Pero un RSI de 75
solo dice que **las subas recientes fueron más grandes que las bajas**: no dice que el precio vaya
a bajar.

**VWAP.** El precio promedio del día ponderado por el volumen: cada precio pesa según cuánto se
operó a ese precio. Necesita el volumen de cada vela.

**Lo que el motor no tiene.** El motor **no calcula hoy ni el RSI ni el VWAP**: no hay código para
ellos en el módulo de indicadores (la guía de las pruebas contra TradingView lo dice del RSI: "no
existe en el motor"). La UI tampoco los calcula: nunca calcula lógica de trading (CLAUDE.md §1). Por eso
esta lección **no abre ninguna capa** de RSI ni de VWAP. Cuando el motor los agregue, con su prueba
contra la fórmula publicada, esta lección abrirá su capa.

> **Probá antes de seguir:** en las últimas 14 velas, la ganancia media fue 15 puntos y la pérdida
> media 10. ¿Cuánto da el RSI?
>
> <details><summary>Ver la respuesta</summary>
>
> RS = 15 ÷ 10 = 1,5. RSI = 100 − 100 ÷ 2,5 = 100 − 40 = **60**.
>
> </details>

## Interactivo

Una tabla del motor con la volatilidad realizada y el ATR diario de los últimos días
(`daily_indicator.v1`) y el tercio de cada día (`daily_vol_tercile.v1`). No hay capa de RSI ni de
VWAP porque el motor no los calcula.

## Ejemplo resuelto

Cuatro retornos diarios: +1 %, −1 %, +1 % y −1 % (una ventana de 4 para hacerlo a mano; el motor
usa 20).

- Media: 0.
- Desvíos al cuadrado: 4 × 0,01² = 0,0004.
- Muestral: se divide por n − 1 = 3 → 0,000133.
- Raíz: **0,0115**, o sea 1,15 % por día.

## Ejercicio

Ordená de menos a más "agitado" estos tres días del mismo índice, con su volatilidad realizada:
0,006, 0,018 y 0,011. Después explicá por qué el tercio (bajo, medio, alto) se calcula contra los 3
años anteriores y no contra toda la historia.

<details><summary>Ver la solución</summary>

0,006 < 0,011 < 0,018. El tercio se mide contra la historia **anterior** a ese día (3 años): usar
toda la historia metería datos del futuro (mirar el futuro, CLAUDE.md §5b).

</details>

## Examen

7 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada por ahora. El temario decía "RSI y VWAP", pero el motor no los calcula: no se abre una
herramienta que no existe.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Ventana de la volatilidad realizada | 20 retornos diarios (`20`) | `config/indicators.toml` `daily.realized_vol_window` | A |
| Tercios contra los años anteriores | 3 (`3`) | `config/indicators.toml` `daily.vol_tercile_years` | A |
| Desvío estándar muestral | `desvío estándar muestral de los últimos n retornos` | `engine/crates/indicators/src/volatility.rs` | A |
| Retorno diario | `cierre_t / cierre_{t−1} − 1` | `engine/crates/indicators/src/volatility.rs` | A |
| R019: alfa por mes, p y Sharpe frente a comprar y mantener | `+0.192 % (p 0.171; 0.245 vs 0.261)` | `docs/analysis/registro-de-resultados.csv` `R019` | A |
| R019: hacía falta p menor que 0,0125 | `hacía falta < 0,0125` | `docs/analysis/2026-11-ronda3-volatilidad-y-numeros-redondos.md` | A |
| El motor no tiene RSI | `no existe en el motor` | `docs/guides/golden-tradingview.md` | A |
| Indicadores que sí tiene el motor (sin RSI ni VWAP) | `pub use moving_average::{ema, ema_values, sma, sma_values};` | `engine/crates/indicators/src/lib.rs` | A |
| VWAP: precio del día ponderado por el volumen | `Precio promedio del día ponderado por el volumen` | `docs/GLOSARIO.md` | A |
| RSI: fórmula, 14 velas, 70 y 30, Wilder (1978) | — | [TradingView, *Relative Strength Index (RSI)*](https://in.tradingview.com/support/solutions/43000502338-relative-strength-index-rsi/), leída el 26-11-2026 | B |
