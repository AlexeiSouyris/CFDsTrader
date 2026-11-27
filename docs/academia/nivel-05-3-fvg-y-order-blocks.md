# 5.3 FVG y order blocks

> **Nivel 5 · Liquidez.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa en
> simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Saber qué son un [FVG](../GLOSARIO.md#fvg) y un [order block](../GLOSARIO.md#order-block) con la
regla del motor, y leer sus estadísticas de toque, mitad y llenado con su intervalo de confianza.

## La idea

**FVG (fair value gap, "hueco").** Tres velas seguidas, `i−2`, `i−1` e `i`. Para el lado alcista:

```text
FVG alcista en la vela i:  L[i] > H[i−2]      zona = [H[i−2], L[i]]
toque en t:   L[t] ≤ techo de la zona y C[t] ≥ piso   (entra sin llenarlo)
lleno:        una vela posterior cierra por debajo de H[i−2]   (deja de valer)
mitad:        el precio medio de la zona
```

Es decir: la vela del medio se movió tan rápido que entre el máximo de la primera y el mínimo de la
tercera quedó un tramo sin superposición. El bajista es el espejo. En el puntaje de la estrategia
en uso, tocar un FVG sin llenar de las **últimas 50 velas** suma 1 (el mismo punto que un order
block: "FVG u order block").

**Order block (OB).** Para el lado alcista, por cada **ruptura de estructura** alcista (BOS o
CHoCH, lección 5.2):

```text
impulso k = la última vela alcista entre el pivote roto y la ruptura con cuerpo ≥ 1,5 × ATR
OB        = la última vela bajista antes de k;  zona = [L, H] de esa vela
vigente   desde el cierre de la vela de la ruptura, hasta que una vela cierre por debajo de L
toque     L[t] ≤ H del OB y C[t] ≥ L del OB
```

La spec habla de "un BOS"; el motor cuenta cualquier ruptura (BOS o CHoCH). El OB existe **recién
cuando se conoce la ruptura**: antes, nadie sabe cuál fue "la última vela bajista antes del
impulso".

**Estadísticas (solo informativas).** Para cada FVG de 15 m, 1 h y 1 d el motor registra la
primera vela que lo **toca**, la que llega a la **mitad** y la que lo **llena**, la reacción a 4 y
8 velas después del primer toque (en ATR) y si **vence** sin llenarse a las 500 velas. Por
instrumento, timeframe, dirección y tamaño muestra la tasa con su **intervalo de Wilson al 95 %**.
Con menos de **30 casos** dice "datos insuficientes" en lugar de una tasa: una tasa con 8 casos no
dice casi nada.

**Un dato real.** En US100, de 2013 a 2025, hubo 59.636 primeros toques de FVG de 15 m; en 13.443
de ellos (≈ 22,5 %) la vela del toque ya lo llenó. Contar así es lo que permite, en la lección 5.4,
preguntar si el FVG sirve.

> **Probá antes de seguir:** vela `i−2` con máximo 6.010 y vela `i` con mínimo 6.018. ¿Hay FVG?
> ¿Dónde está su mitad?
>
> <details><summary>Ver la respuesta</summary>
>
> Sí: 6.018 > 6.010, FVG alcista con zona [6.010, 6.018]. Mitad: 6.014.
>
> </details>

## Interactivo

La tabla de estadísticas de FVG por instrumento y timeframe (`liquidity_stats.v1`): tasa de toque,
de mitad y de llenado con su intervalo, cantidad de casos y la reacción, al lado de las capas de FVG
y order blocks sobre un gráfico real.

## Ejemplo resuelto

FVG alcista con zona [6.010, 6.018] creado en la vela 100.

- Vela 104: mínimo 6.016, cierre 6.021 → **primer toque** (6.016 ≤ 6.018 y 6.021 ≥ 6.010).
- Vela 109: mínimo 6.013 → llega a la **mitad** (6.014).
- Vela 115: cierra en 6.008 < 6.010 → **lleno**: desde acá ya no vale.

Order block: el pivote alto roto estaba en 6.040, la ruptura fue en la vela 230 y el ATR es 20. La
última vela alcista con cuerpo ≥ 30 puntos (1,5 × 20) antes de la ruptura es la 228; la última
vela bajista antes de ella es la 226, con L 6.002 y H 6.011. **OB en [6.002, 6.011]**, vigente
desde el cierre de la 230 hasta que una vela cierre abajo de 6.002.

## Ejercicio

En un replay de práctica, encontrá 3 FVG y anotá para cada uno: la vela que lo creó, su zona, el
primer toque y la vela que lo llenó (o si sigue sin llenar).

<details><summary>Ver la solución</summary>

Comparalo con la capa de FVG del motor. Los errores típicos: tomar la vela `i−1` en lugar de la
`i−2`, contar como llenado un toque del borde (hace falta un **cierre** del otro lado) y marcar el
FVG antes de que cierre la vela `i`.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Capas de FVG y order blocks** del motor.
- **Estadísticas de FVG**: toque, mitad, llenado y reacción con su intervalo de Wilson.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| FVG alcista: mínimo de `i` arriba del máximo de `i−2` | `L[i] > H[i−2]` | `docs/spec/fase-1-especificacion.md` §5.4 | A |
| Se llena al cerrar más allá del borde lejano | `Se llena (y deja de valer) cuando una vela cierra por debajo de H[i−2]` | `docs/spec/fase-1-especificacion.md` §5.4 | A |
| Toque: entra a la zona sin llenarla | `L[t] ≤ techo de la zona y C[t] ≥ piso` | `engine/crates/liquidity/src/fvg.rs` | A |
| FVG de las últimas 50 velas | 50 (`50`) | `config/liquidity.toml` `fvg.lookback_bars` | A |
| Puntaje de «FVG u order block» | +1 (`1`) | `config/strategy.toml` `score.fvg_or_order_block` | A |
| OB: cuerpo mínimo del impulso | 1,5 ATR (`1.5`) | `config/liquidity.toml` `order_blocks.min_impulse_body_atr` | A |
| OB: ATR de 14 velas | 14 (`14`) | `config/liquidity.toml` `order_blocks.atr_period` | A |
| OB: cuenta cualquier ruptura | `ruptura (BOS o CHoCH)` | `engine/crates/liquidity/src/order_blocks.rs` | A |
| Reacción medida a 4 y 8 velas | `reaction_bars = [4, 8]` | `config/liquidity.toml` | A |
| Un FVG vence sin llenarse a las 500 velas | 500 (`500`) | `config/liquidity.toml` `fvg_stats.max_age_bars` | A |
| Intervalo de Wilson al 95 % (z) | 1,96 (`1.96`) | `config/liquidity.toml` `stats.z` | A |
| Menos casos que esto: «datos insuficientes» | 30 (`30`) | `config/liquidity.toml` `stats.min_cases` | A |
| US100: primeros toques de FVG de 15 m (2013–2025) | `59.636` | `docs/analysis/2026-11-programa-5-resultados.md` §2 | A |
| US100: llenados en la misma vela del toque | `13.443` | `docs/analysis/2026-11-programa-5-resultados.md` §2 | A |
