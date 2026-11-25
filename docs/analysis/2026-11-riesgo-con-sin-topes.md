# Backtest de cartera con y sin los topes de riesgo aprobados

- **Fecha:** 25-11-2026 (corrida a las 18:03 UTC, código `39f6235` + los cambios de #689).
- **Issue:** #689. Valores aprobados por el dueño el 25-11-2026 («1, 2 y 7 apruebo la recomendacion.», en https://github.com/AlexeiSouyris/CFDsTrader/issues/671#issuecomment-7169989627); detalle en `docs/analysis/2026-11-riesgo-propuesta.md`.
- **Tipo:** **descriptivo**. No es una prueba de hipótesis: no vota, no tiene criterio de aprobación y **no suma al N** del Sharpe deflactado (regla de `tools/research/check_dsr_trials.py`: la fila va con `programa` «descriptivo (riesgo)»). Fila **R039** del registro.
- **Nada de esto es consejo de inversión.** Es una simulación sobre el pasado y no garantiza nada del futuro.

## Qué se corrió

- **Estrategia:** la estrategia en uso de 15 minutos, sin ningún cambio (la misma de R001–R003).
- **Cartera:** US500 + US100 + US30 en **una sola cuenta** (`run_portfolio`: capital realizado + no realizado, margen libre, máximo de 5 posiciones), como las filas de cartera del registro (R014–R020).
- **Período:** 01-02-2013 a 01-11-2025 (153 meses), antes del tramo fuera de muestra.
- **Datos:** velas de 15 m y 1 h armadas desde los futuros de Databento, preparadas igual que `diagnostico --fuente futuros` (validación, horario, vencimientos). Sin filtro de spread ni calendario de noticias en ese período (no hay datos; la corrida lo avisa) y sin ajuste de noticias del puntaje (no hay interpretaciones antes del corte del modelo).
- **Costos:** spread, slippage y comisión de `config/`, financiación con el escenario USD (EFFR hasta 2018-03, SOFR desde 2018-04: lo que IG confirmó para estos índices, #655) menos la base del futuro.
- **Capital inicial:** 10 000 EUR. Kill switch del backtest con reactivación a los 20 días hábiles (`config/backtest.toml`, solo backtest).
- **Cinco variantes, todo igual menos `config/risk.toml`:**
  - `sin_topes`: lo de #670 y #671 apagado (el comportamiento anterior);
  - `solo_gaps`: solo `gap_atr = 0.25`;
  - `solo_grupo`: solo el grupo `us_equity_index` con tope de 1,5 %;
  - `solo_caida`: solo la caída gradual (5 %, 1), (10 %, 0,5), (12,5 %, 0,25);
  - `con_topes`: `config/risk.toml` tal cual quedó en #689.
- **Monte Carlo:** `[montecarlo]` de `config/research.toml` (10 000 caminos, bloques circulares de 5 meses, semilla fija) sobre los 153 retornos mensuales de cada variante. Verificado con `montecarlo_report` sobre el CSV guardado: da los mismos números.

## Resultados

| Variante | Operaciones | Capital final (EUR) | Retorno total | Sharpe (diario, anualizado) | Caída máxima | Monte Carlo caída p50 / p95 | Kill switch | Riesgo medio por operación |
|---|---|---|---|---|---|---|---|---|
| `sin_topes` | 978 | 242 | −97,6 % | −1,69 | 97,6 % | 97,5 % / 99,6 % | 28 | 0,98 % |
| `solo_gaps` | 1 248 | 272 | −97,3 % | −1,65 | 97,3 % | 97,3 % / 99,5 % | 28 | 0,99 % |
| `solo_grupo` | 1 337 | 279 | −97,2 % | −1,81 | 97,2 % | 97,2 % / 99,4 % | 28 | 0,81 % |
| `solo_caida` | 1 204 | 671 | −93,3 % | −1,70 | 93,3 % | 93,1 % / 98,2 % | 21 | 0,52 % |
| **`con_topes`** | **1 352** | **841** | **−91,6 %** | **−1,95** | **91,6 %** | **91,4 % / 97,4 %** | **16** | **0,52 %** |

Qué hizo cada regla (en `con_topes`, salvo que se diga otra cosa):

| Regla | Efecto |
|---|---|
| Tope por grupo (1,5 %) | 198 operaciones **achicadas** y 45 **rechazadas** (`group_risk_limit`). Sola (`solo_grupo`): 389 achicadas y 280 rechazadas |
| Colchón por gaps (0,25 ATR) | **todas** las operaciones salen más chicas: en promedio el colchón es el 14,8 % del riesgo de cada una (sola: 14,3 %) |
| Caída gradual | 1 073 de 1 352 operaciones se abrieron con el riesgo reducido (factor < 1); sola, 973 de 1 204 |

Rechazos del riesgo por motivo (todas las señales del período):

| Motivo | `sin_topes` | `con_topes` |
|---|---|---|
| Tamaño por debajo del mínimo del broker | 15 366 | 16 431 |
| Stop demasiado lejos | 9 109 | 9 883 |
| Pérdida diaria | 339 | 73 |
| Drawdown máximo (kill switch) | 28 | 16 |
| Margen insuficiente | 36 | 0 |
| Tope del grupo | 0 | 45 |

## Cómo leerlo (sin promesas)

1. **La estrategia pierde con o sin topes.** Los topes no la convierten en ganadora: el resultado con topes es −91,6 % en 12,75 años. Es lo mismo que ya decían R001–R009 (los costos hacen la pérdida). Los topes **no cambian ninguna señal**, solo el tamaño.
2. **Con los topes se pierde menos plata y más despacio.** El capital final es 841 EUR frente a 242 EUR, la caída máxima 91,6 % frente a 97,6 % y el kill switch salta 16 veces en vez de 28. Casi todo eso lo hace la **caída gradual**: con la cuenta en caída, cada operación arriesga la mitad (riesgo medio 0,52 % frente a 0,98 %). El grupo y los gaps, solos, casi no mueven el resultado final.
3. **El Sharpe empeora (−1,95 frente a −1,69).** Con una estrategia que pierde, achicar el tamaño en las rachas malas reduce la pérdida en euros pero no la mejora por unidad de riesgo: las operaciones siguen perdiendo y la volatilidad diaria baja menos que la media. No hay que leer el Sharpe como "mejor" o "peor" gestión: describe una estrategia que pierde.
4. **Hay más operaciones con topes (1 352 frente a 978)** porque la cuenta se achica más despacio: sin topes, el capital cae tanto que la mayoría de las señales no llega al **tamaño mínimo** del broker y se rechaza. El efecto es de camino (path-dependent): las variantes no operan las mismas señales.
5. **El Monte Carlo dice lo mismo**: la ruina (tocar −30 %) aparece en prácticamente todos los caminos de las cinco variantes. El p95 de la caída baja de 99,6 % a 97,4 %.

## Limitaciones

- **El Monte Carlo remuestrea los retornos mensuales ya realizados**: no vuelve a aplicar la caída gradual ni el tope del grupo en cada camino. Mide la dispersión de la serie que salió, no la de las reglas.
- **El kill switch se reactiva a los 20 días hábiles** (solo en backtest, #277), y con él el pico desde el que se mide la caída gradual. En vivo no se reactiva solo: en paper, después de un kill switch no se opera más hasta la decisión del dueño.
- **El broker simulado mide la caída desde un pico que solo actualiza al decidir** (#691); este backtest lo actualiza en cada vela. En paper la reducción por caída puede actuar más tarde que acá hasta que se arregle #691.
- La volatilidad objetivo **no** está en ninguna variante (sigue apagada; falta #692).
- Sin filtro de spread ni de noticias de alto impacto en este período (no hay datos), igual que R001–R003.
- Solo US500, US100 y US30: US2000 está en el grupo pero no tiene costos confirmados (R008 usó costos supuestos) y no entra en la cartera.

## Cómo repetirlo

```text
cd engine
DATABASE_URL=...  # sesión de solo lectura
cargo run --release -p cfds-backtest --example riesgo_con_sin_topes -- \
    2013-02-01T00:00:00Z 2025-11-01T00:00:00Z 10000 \
    --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv \
    --csv ../docs/analysis/2026-11-riesgo-con-sin-topes-retornos.csv
cargo run --release -p cfds-backtest --example montecarlo_report -- \
    ../docs/analysis/2026-11-riesgo-con-sin-topes-retornos.csv con_topes
```

Los retornos mensuales de las cinco variantes quedan en `docs/analysis/2026-11-riesgo-con-sin-topes-retornos.csv`.
