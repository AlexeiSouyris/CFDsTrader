# Fase 3: comprar y mantener sobre datos reales, calculado a mano

Criterio de aprobación de la Fase 3 (`docs/ROADMAP.md`): *comprar y mantener da un resultado calculable a mano*. Sale de #76 y lo verifica el test `buy_and_hold_on_real_de40_matches_the_hand_calculation` (`engine/crates/backtest/tests/setup.rs`).

## Datos

- **Instrumento:** DE40, CFD al contado de 1 € por punto (`IX.D.DAX.IFE.IP`), cuenta demo de IG.
- **Velas:** de 15m, a precio medio, del 09-11-2026 00:00 UTC al 20-11-2026 22:00 UTC. Se cargaron con `ig_backfill` por el circuito de ingesta (validación → PostgreSQL).
- **Primera vela:** 2026-11-09 00:00 UTC, apertura **25669,3**.
- **Última vela:** 2026-11-20 21:45 UTC, cierre **24614,2**.
- **Tamaño:** 1 (1 € por punto).

## Parámetros, todos de la configuración

| Parámetro | Valor | Fuente |
|---|---|---|
| Spread típico | 1,8 puntos | `instruments.toml` (`ig-es-indices`: el mayor entre el medido y el publicado, #67) |
| Tick | 0,1 | `instruments.toml` (`ig-demo-api`, `decimalPlacesFactor` = 1) |
| Slippage | 0,1 × spread + 1 tick | `backtest.toml` |
| Comisión | 0 (índice) | `ig-es-costes` |
| Financiación | (€STR + 2,5 %) / 360 por noche, corte a las 23:00 de Madrid | `ig-es-costes` |
| €STR | 2,441 % | `ecb-estr` (BCE, valor del 19-11-2026) |

## Cálculo

1. **Costo por ejecución:** spread/2 + slippage = 0,9 + (0,18 + 0,1) = **1,18 puntos**.
2. **Entrada (compra):** 25669,3 + 1,18 = **25670,48**.
3. **Salida (venta):** 24614,2 − 1,18 = **24613,02**.
4. **P/L bruto:** 1 × (24613,02 − 25670,48) = **−1057,46 €**.
5. **Noches:** en noviembre, las 23:00 de Madrid son las 22:00 UTC. Se cruzan los cortes del 9 al 20, contando el fin de semana: **12**. La posición se cierra al fin de la última vela, 22:00 UTC del 20, justo en el corte; el motor lo cobra. Ver supuestos.
6. **Financiación:** 12 × 25669,3 × 1 × (2,441 % + 2,5 %) / 360 = 12 × 3,52311425 = **42,2773371 €**.
7. **P/L neto:** −1057,46 − 42,2773371 = **−1099,7373371 €**.

El motor da exactamente estos valores (`cargo run -p cfds-backtest --example backtest_run -- DE40 2026-11-09T00:00:00Z 2026-11-21T00:00:00Z 10000 1`).

## Supuestos (los del motor, documentados en `runner.rs`)

- **Precio de la financiación:** se usa el precio medio de entrada como "cierre" de cada noche. IG usa el cierre de cada día; para 12 días, la diferencia es de centavos por noche.
- **Salida justo en el corte:** si la salida cae exactamente en el corte de las 23:00, se cobra esa noche.
- **€STR constante:** se usa constante en todo el recorrido. Sirve para períodos cortos como este; para backtests largos hace falta la serie diaria del BCE (pendiente).
